'use strict';

const { createConnection, ProposedFeatures, TextDocuments, TextDocumentSyncKind, ResponseError,
  ErrorCodes } = require('vscode-languageserver/node');
const { TextDocument } = require('vscode-languageserver-textdocument');
const { Analysis } = require('./analysis');
const { Features, TOKEN_TYPES, TOKEN_MODIFIERS } = require('./features');
const { HintWorkspace, isInterface } = require('./hints');

const connection = createConnection(ProposedFeatures.all);
const documents = new TextDocuments(TextDocument);
const hints = new HintWorkspace(documents);
const cache = new Map(), timers = new Map();
let warnings = true, snippetSupport = false;
let inlayHints = { types: true, parameters: true };
let refreshInlayHints = false;
let refreshSemanticTokens = false;
let hierarchicalSymbols = false;
let workspaceFolderSupport = false;

connection.onInitialize(params => {
  hints.setRoots(params.workspaceFolders?.map(folder => folder.uri) || (params.rootUri ? [params.rootUri] : []));
  snippetSupport = params.capabilities.textDocument?.completion?.completionItem?.snippetSupport === true;
  refreshInlayHints = params.capabilities.workspace?.inlayHint?.refreshSupport === true;
  refreshSemanticTokens = params.capabilities.workspace?.semanticTokens?.refreshSupport === true;
  hierarchicalSymbols = params.capabilities.textDocument?.documentSymbol?.hierarchicalDocumentSymbolSupport === true;
  workspaceFolderSupport = params.capabilities.workspace?.workspaceFolders === true;
  return { capabilities: {
  workspace: { workspaceFolders: { supported: true, changeNotifications: true } },
  textDocumentSync: TextDocumentSyncKind.Incremental,
  definitionProvider: true, typeDefinitionProvider: true, referencesProvider: true, hoverProvider: true,
  codeActionProvider: { codeActionKinds: ['quickfix'] }, inlayHintProvider: true, selectionRangeProvider: true,
  documentHighlightProvider: true, documentSymbolProvider: true, foldingRangeProvider: true,
  renameProvider: { prepareProvider: true }, completionProvider: { triggerCharacters: ['.', '@'] },
  signatureHelpProvider: { triggerCharacters: ['(', ','], retriggerCharacters: [','] },
  semanticTokensProvider: { legend: { tokenTypes: TOKEN_TYPES, tokenModifiers: TOKEN_MODIFIERS }, full: true }
}, serverInfo: { name: 'Prim Language Server', version: require('../package.json').version } };
});

function features(uri) {
  const document = documents.get(uri);
  if (!document) return null;
  let entry = cache.get(uri);
  if (!entry || entry.document.version !== document.version) {
    // Keep a versioned snapshot: TextDocuments mutates its live document on edits.
    const snapshot = TextDocument.create(uri, document.languageId, document.version, document.getText());
    const context = hints.context(snapshot);
    entry = new Features(snapshot, isInterface(snapshot) ? context.analysis : new Analysis(snapshot.getText(), null, context));
    cache.set(uri, entry);
  }
  return entry;
}

function publish(document) {
  try {
    const feature = features(document.uri);
    if (feature) connection.sendDiagnostics({ uri: document.uri, version: feature.document.version, diagnostics: feature.diagnostics(warnings) });
  } catch (error) { connection.console.error(`Analysis failed: ${error.stack || error}`); }
}

documents.onDidChangeContent(({ document }) => {
  if (isInterface(document)) { refreshDocuments(); return; }
  clearTimeout(timers.get(document.uri));
  timers.set(document.uri, setTimeout(() => { timers.delete(document.uri); publish(document); }, 150));
});
documents.onDidClose(({ document }) => {
  clearTimeout(timers.get(document.uri)); timers.delete(document.uri); cache.delete(document.uri);
  connection.sendDiagnostics({ uri: document.uri, diagnostics: [] });
  if (isInterface(document)) refreshDocuments();
});
function refreshDocuments() {
  cache.clear();
  for (const document of documents.all()) {
    clearTimeout(timers.get(document.uri));
    timers.set(document.uri, setTimeout(() => { timers.delete(document.uri); publish(document); }, 150));
  }
  if (refreshInlayHints) connection.languages.inlayHint.refresh().catch(error => connection.console.error(String(error)));
  if (refreshSemanticTokens) connection.languages.semanticTokens.refresh().catch(error => connection.console.error(String(error)));
}
connection.onDidChangeWatchedFiles(refreshDocuments);
connection.onInitialized(() => {
  if (!workspaceFolderSupport) return;
  connection.workspace.onDidChangeWorkspaceFolders(event => {
    const { pathToFileURL } = require('node:url');
    const removed = new Set(event.removed.map(folder => folder.uri));
    hints.setRoots([...hints.roots.map(root => pathToFileURL(root).href).filter(uri => !removed.has(uri)), ...event.added.map(folder => folder.uri)]);
    refreshDocuments();
  });
});
connection.onDidChangeConfiguration(({ settings }) => {
  warnings = settings?.prim?.diagnostics?.warnings !== false;
  inlayHints = { types: settings?.prim?.inlayHints?.types !== false, parameters: settings?.prim?.inlayHints?.parameters !== false };
  for (const document of documents.all()) publish(document);
  if (refreshInlayHints) connection.languages.inlayHint.refresh().catch(error => connection.console.error(String(error)));
});

const request = fn => params => {
  const feature = features(params.textDocument.uri);
  return feature ? fn(feature, params) : null;
};
connection.onDefinition(request((f, p) => f.definition(p.position)));
connection.onTypeDefinition(request((f, p) => f.typeDefinition(p.position)));
connection.onCodeAction(request((f, p) => f.codeActions(p.range, p.context, warnings)));
connection.onSelectionRanges(request((f, p) => f.selectionRanges(p.positions)));
connection.languages.inlayHint.on(request((f, p) => f.inlayHints(p.range, inlayHints)));
connection.onReferences(request((f, p) => f.references(p.position, p.context.includeDeclaration)));
connection.onHover(request((f, p) => f.hover(p.position)));
connection.onDocumentHighlight(request((f, p) => f.highlights(p.position)));
connection.onPrepareRename(request((f, p) => f.prepareRename(p.position)));
connection.onRenameRequest(request((f, p) => {
  try { return f.rename(p.position, p.newName); }
  catch (error) { throw new ResponseError(ErrorCodes.InvalidParams, error.message); }
}));
connection.onCompletion(request((f, p) => f.completion(p.position, snippetSupport)));
connection.onSignatureHelp(request((f, p) => f.signatureHelp(p.position)));
connection.onDocumentSymbol(request(f => {
  const symbols = f.documentSymbols();
  if (hierarchicalSymbols) return symbols;
  const flat = [];
  const visit = (items, containerName) => {
    for (const item of items) {
      flat.push({ name: item.name, kind: item.kind, containerName, location: { uri: f.document.uri, range: item.selectionRange } });
      if (item.children) visit(item.children, item.name);
    }
  };
  visit(symbols);
  return flat;
}));
connection.onFoldingRanges(request(f => f.foldingRanges()));
connection.languages.semanticTokens.on(request(f => f.semanticTokens()));
connection.onShutdown(() => { for (const timer of timers.values()) clearTimeout(timer); });
documents.listen(connection);
connection.listen();
