'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { spawn, fork } = require('node:child_process');
const path = require('node:path');
const { createMessageConnection, StreamMessageReader, StreamMessageWriter, IPCMessageReader, IPCMessageWriter } = require('vscode-jsonrpc/node');

for (const transport of ['stdio', 'ipc']) test(`real LSP lifecycle (${transport}): navigation, tokens, edits, diagnostics, close and shutdown`, { timeout: 10000 }, async t => {
  const server = process.env.PRIM_TEST_SERVER || path.join(__dirname, '../server/server.js');
  const child = transport === 'stdio'
    ? spawn(process.execPath, [server, '--stdio'], { stdio: ['pipe', 'pipe', 'pipe'] })
    : fork(server, ['--node-ipc'], { stdio: ['pipe', 'pipe', 'pipe', 'ipc'] });
  let stderr = '';
  child.stderr.on('data', chunk => { stderr += chunk; });
  const connection = transport === 'stdio'
    ? createMessageConnection(new StreamMessageReader(child.stdout), new StreamMessageWriter(child.stdin))
    : createMessageConnection(new IPCMessageReader(child), new IPCMessageWriter(child));
  t.after(() => { connection.dispose(); if (child.exitCode === null) child.kill(); });
  child.once('exit', () => connection.dispose());
  t.signal.addEventListener('abort', () => { connection.dispose(); child.kill(); }, { once: true });
  const serverMessages = [];
  connection.onNotification('window/logMessage', event => serverMessages.push(event.message));
  const reports = [], listeners = [];
  let hintRefreshes = 0;
  connection.onRequest('workspace/inlayHint/refresh', () => { hintRefreshes++; return null; });
  connection.onNotification('textDocument/publishDiagnostics', report => {
    reports.push(report);
    for (const listener of [...listeners]) listener(report);
  });
  function diagnostics(version, predicate = () => true, uriFilter) {
    const ready = reports.find(r => r.version === version && (!uriFilter || r.uri === uriFilter) && predicate(r));
    if (ready) return Promise.resolve(ready);
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error(`Missing diagnostics ${uriFilter || ''} v${version}: ${serverMessages.join('\n')}`)), 3000);
      const listener = report => {
        if (report.version === version && (!uriFilter || report.uri === uriFilter) && predicate(report)) { clearTimeout(timer); listeners.splice(listeners.indexOf(listener), 1); resolve(report); }
      };
      listeners.push(listener);
    });
  }
  connection.listen();
  try {
    const init = await connection.sendRequest('initialize', { processId: process.pid, rootUri: null, capabilities: {
      textDocument: { completion: { completionItem: { snippetSupport: true } }, documentSymbol: { hierarchicalDocumentSymbolSupport: true } },
      workspace: { inlayHint: { refreshSupport: true } }
    } });
    assert.ok(init.capabilities.definitionProvider);
    assert.ok(init.capabilities.typeDefinitionProvider);
    assert.ok(init.capabilities.inlayHintProvider);
    assert.ok(init.capabilities.codeActionProvider);
    assert.ok(init.capabilities.semanticTokensProvider.legend.tokenTypes.includes('type'));
    await connection.sendNotification('initialized', {});
    const uri = 'file:///protocol.pr', text = 'let Point = $[](x) { @{ let value = x; } };\nlet p = Point(1);\np.value;';
    const textDocument = { uri };
    await connection.sendNotification('textDocument/didOpen', { textDocument: { uri, languageId: 'prim', version: 1, text } });
    assert.deepEqual((await diagnostics(1)).diagnostics, []);
    const position = { line: 2, character: 3 };
    const definition = await connection.sendRequest('textDocument/definition', { textDocument, position });
    assert.equal(definition.range.start.line, 0);
    const hover = await connection.sendRequest('textDocument/hover', { textDocument, position: { line: 1, character: 10 } });
    assert.match(hover.contents.value, /Uppercase constructor/);
    const refs = await connection.sendRequest('textDocument/references', { textDocument, position, context: { includeDeclaration: true } });
    assert.equal(refs.length, 2);
    const rename = await connection.sendRequest('textDocument/rename', { textDocument, position, newName: 'coordinate' });
    assert.equal(rename.documentChanges[0].edits.length, 2);
    await assert.rejects(connection.sendRequest('textDocument/rename', { textDocument, position, newName: 'return' }));
    const tokens = await connection.sendRequest('textDocument/semanticTokens/full', { textDocument });
    assert.ok(tokens.data.length > 0);
    assert.ok((await connection.sendRequest('textDocument/documentSymbol', { textDocument })).some(s => s.name === 'Point' && s.kind === 5));
    assert.ok((await connection.sendRequest('textDocument/documentHighlight', { textDocument, position })).length === 2);
    const completion = await connection.sendRequest('textDocument/completion', { textDocument, position: { line: 2, character: 2 } });
    assert.ok(completion.items.some(item => item.label === 'value'));
    const signature = await connection.sendRequest('textDocument/signatureHelp', { textDocument, position: { line: 1, character: 14 } });
    assert.equal(signature.signatures[0].label, 'Point(x)');
    const typeDefinition = await connection.sendRequest('textDocument/typeDefinition', { textDocument, position: { line: 2, character: 0 } });
    assert.equal(typeDefinition.range.start.character, 4);
    const wholeRange = { start: { line: 0, character: 0 }, end: { line: 3, character: 0 } };
    const hints = await connection.sendRequest('textDocument/inlayHint', { textDocument, range: wholeRange });
    assert.ok(hints.some(hint => hint.kind === 1 && hint.label[1].value === 'Point'));
    const selections = await connection.sendRequest('textDocument/selectionRange', { textDocument, positions: [position] });
    assert.ok(selections[0].parent);
    const outline = await connection.sendRequest('textDocument/documentSymbol', { textDocument });
    assert.ok(outline.find(s => s.name === 'Point').children.some(s => s.name === 'x'));

    await connection.sendNotification('textDocument/didChange', { textDocument: { uri, version: 2 }, contentChanges: [{ range: { start: { line: 2, character: 0 }, end: { line: 2, character: 8 } }, text: 'missing;' }] });
    assert.ok((await diagnostics(2)).diagnostics.some(d => d.code === 'UndefinedName' && d.severity === 1));
    const newTokens = await connection.sendRequest('textDocument/semanticTokens/full', { textDocument });
    assert.notDeepEqual(newTokens.data, tokens.data);
    await connection.sendNotification('textDocument/didChange', { textDocument: { uri, version: 3 }, contentChanges: [{ text: 'let unused = 1;' }] });
    assert.ok((await diagnostics(3)).diagnostics.some(d => d.severity === 2));
    const actions = await connection.sendRequest('textDocument/codeAction', { textDocument, range: wholeRange, context: { diagnostics: (await diagnostics(3)).diagnostics } });
    assert.ok(actions.some(a => a.edit.documentChanges[0].textDocument.version === 3 && a.title.includes('_unused')));
    const snippets = await connection.sendRequest('textDocument/completion', { textDocument, position: { line: 0, character: 15 } });
    assert.ok(snippets.items.some(item => item.insertTextFormat === 2));
    await connection.sendNotification('workspace/didChangeConfiguration', { settings: { prim: { diagnostics: { warnings: false }, inlayHints: { types: false, parameters: false } } } });
    assert.deepEqual((await diagnostics(3, r => !r.diagnostics.length)).diagnostics, []);
    assert.deepEqual(await connection.sendRequest('textDocument/inlayHint', { textDocument, range: wholeRange }), []);
    assert.ok(hintRefreshes > 0);
    await connection.sendNotification('textDocument/didChange', { textDocument: { uri, version: 4 }, contentChanges: [{ text: 'let x = 1; let f = $[]() { x };' }] });
    const captureDiagnostics = (await diagnostics(4)).diagnostics;
    const captureActions = await connection.sendRequest('textDocument/codeAction', { textDocument, range: wholeRange, context: { diagnostics: captureDiagnostics } });
    assert.equal(captureActions.filter(a => a.title.includes('capture list')).length, 2);
    const standardLibrary = 'let x = std.random.int(1, 10); std.term.size();';
    await connection.sendNotification('textDocument/didChange', { textDocument: { uri, version: 5 }, contentChanges: [{ text: standardLibrary }] });
    assert.deepEqual((await diagnostics(5)).diagnostics, []);
    const standardCompletions = await connection.sendRequest('textDocument/completion', { textDocument, position: { line: 0, character: standardLibrary.indexOf('size') } });
    assert.ok(standardCompletions.items.some(item => item.label === 'size'));
    assert.ok(standardCompletions.items.some(item => item.label === 'poll_key'));
    const randomSignature = await connection.sendRequest('textDocument/signatureHelp', { textDocument, position: { line: 0, character: standardLibrary.indexOf('10') } });
    assert.equal(randomSignature.signatures[0].label, 'std.random.int(lo: int, hi: int) -> int');
    await connection.sendNotification('textDocument/didChange', { textDocument: { uri, version: 6 }, contentChanges: [{ text: 'std.print = 1;' }] });
    assert.deepEqual((await diagnostics(6)).diagnostics, []);
    const typedUri = 'file:///prim-interface-protocol.pr', stubUri = 'file:///prim-interface-protocol.pri';
    const typedSource = 'let read = $[](point) { point.x }; read(@{ let x = 1; });';
    const stub = 'type Point = @{ let x: int; }; let read: $[](point: Point) -> int;';
    await connection.sendNotification('textDocument/didOpen', { textDocument: { uri: typedUri, languageId: 'prim', version: 1, text: typedSource } });
    await connection.sendNotification('textDocument/didOpen', { textDocument: { uri: stubUri, languageId: 'prim-interface', version: 1, text: stub } });
    assert.deepEqual((await diagnostics(1, () => true, stubUri)).diagnostics, []);
    const typedParams = { textDocument: { uri: typedUri }, position: { line: 0, character: typedSource.indexOf('point.x') + 6 } };
    const typedDefinition = await connection.sendRequest('textDocument/definition', typedParams);
    assert.equal(typedDefinition.uri, stubUri);
    const functionParams = { textDocument: { uri: typedUri }, position: { line: 0, character: 5 } };
    assert.match((await connection.sendRequest('textDocument/hover', functionParams)).contents.value, /point: Point.*int/);
    const stubTokens = await connection.sendRequest('textDocument/semanticTokens/full', { textDocument: { uri: stubUri } });
    assert.ok(stubTokens.data.length > 0);
    const typeDef = await connection.sendRequest('textDocument/typeDefinition', { textDocument: { uri: typedUri }, position: { line: 0, character: typedSource.indexOf('point.x') } });
    assert.equal(typeDef.uri, stubUri);
    await connection.sendNotification('textDocument/didChange', { textDocument: { uri: stubUri, version: 2 }, contentChanges: [{ text: stub.replace(/\bint\b/g, 'str') }] });
    assert.match((await connection.sendRequest('textDocument/hover', functionParams)).contents.value, /point: Point.*str/);
    await connection.sendNotification('textDocument/didClose', { textDocument: { uri: stubUri } });
    assert.doesNotMatch((await connection.sendRequest('textDocument/hover', functionParams)).contents.value, /point: Point/);
    await connection.sendNotification('textDocument/didClose', { textDocument: { uri: typedUri } });
    await connection.sendNotification('textDocument/didClose', { textDocument });
    assert.deepEqual((await diagnostics(undefined)).diagnostics, []);
    await connection.sendRequest('shutdown');
    const exited = new Promise(resolve => child.once('exit', resolve));
    await connection.sendNotification('exit');
    assert.equal(await exited, 0, stderr);
  } finally {
    connection.dispose();
    if (child.exitCode === null) child.kill();
  }
});
