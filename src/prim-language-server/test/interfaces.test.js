'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { TextDocument } = require('vscode-languageserver-textdocument');
const { createInterfaces } = require('../server/interfaces');
const { createBuiltins, loadBuiltinSources, validateBuiltins } = require('../server/builtins');
const { Analysis } = require('../server/analysis');
const { Features, TOKEN_TYPES } = require('../server/features');
const { HintWorkspace } = require('../server/hints');

const uri = 'file:///example.pri';
function interfaceFeatures(text) {
  const environment = createInterfaces([...loadBuiltinSources(), { uri, text }]);
  const analysis = environment.analyses.get(uri);
  return { environment, analysis, features: new Features(analysis.document, analysis) };
}

test('root builtin PRI files define every runtime signature and typed nullable results', () => {
  const root = path.resolve(__dirname, '../../../builtin');
  const environment = createInterfaces(loadBuiltinSources(root));
  assert.deepEqual(validateBuiltins(environment), []);
  const builtins = createBuiltins(environment);
  const std = builtins.get('std');
  assert.equal(std.info.fields.get('input').info.result.displayType, 'str | unit');
  assert.equal(std.info.fields.get('len').info.params[0].info.displayType, 'str | closure');
  assert.ok(std.info.fields.get('term').info.fields.get('size').info.result.fields.has('width'));
  assert.ok(std.info.fields.get('gc').info.fields.get('stats').info.result.fields.has('string_bytes'));
  assert.ok(std.info.fields.get('print').location.uri.endsWith('/builtin/std.pri'));
});

test('PRI parses aliases, records, references and function unions without runtime statements', () => {
  const { analysis, features } = interfaceFeatures('type Point = @{ let x: float; let y: float; };\nlet move: $[](&point: Point, delta: float | int) -> Point;');
  assert.deepEqual(features.diagnostics(), []);
  const offset = analysis.text.lastIndexOf('Point');
  const definition = features.definition(analysis.document.positionAt(offset));
  assert.equal(definition.uri, uri);
  assert.equal(analysis.document.offsetAt(definition.range.start), 5);
  assert.ok(features.documentSymbols().some(s => s.name === 'Point' && s.kind === 5));
  const tokens = features.semanticTokens().data;
  assert.ok(tokens.some((v, i) => i % 5 === 3 && v === TOKEN_TYPES.indexOf('type')));
  assert.equal(features.prepareRename(analysis.document.positionAt(offset)), null);
  const completion = features.completion(analysis.document.positionAt(offset + 2));
  assert.ok(completion.items.some(i => i.label === 'Point' && i.kind === 7));
});

test('PRI diagnostics identify unknown types, duplicate declarations, cycles and incomplete syntax', () => {
  for (const [text, code] of [
    ['let value: Missing;', 'UndefinedType'],
    ['type T = int; type T = str;', 'DuplicateType'],
    ['type A = B; type B = A;', 'RecursiveType'],
    ['let f: $[](x: int, x: str) -> unit;', 'DuplicateBinding'],
    ['let value: int = 1;', 'MissingSemicolon'],
    ['std.print(1);', 'InterfaceSyntax']
  ]) assert.ok(interfaceFeatures(text).features.diagnostics().some(d => d.code === code), text);
  for (const text of ['type', 'let f: $[](', 'let f: $[](x:', 'type R = @{ let x:', '}']) assert.doesNotThrow(() => interfaceFeatures(text));
  assert.ok(new Analysis('let x: int = 1;').diagnostics.some(d => d.code === 'IllegalCharacter'));
});

test('source calls use standard-library PRI signatures and jump to their exact declarations', () => {
  const text = 'let size = std.term.size(); size.width;';
  const document = TextDocument.create('file:///example.pr', 'prim', 1, text);
  const f = new Features(document, new Analysis(text));
  const definition = f.definition(document.positionAt(text.indexOf('size()')));
  assert.ok(definition.uri.endsWith('/builtin/std.pri'));
  const field = f.definition(document.positionAt(text.indexOf('width')));
  assert.ok(field.uri.endsWith('/builtin/types.pri'));
  const type = f.typeDefinition(document.positionAt(text.indexOf('size =')));
  assert.ok(type.uri.endsWith('/builtin/types.pri'));
  assert.match(f.hover(document.positionAt(text.indexOf('size()'))).contents.value, /TerminalSize/);
  assert.equal(f.prepareRename(document.positionAt(text.indexOf('width'))), null);
});

test('sibling PRI annotates implementation parameters and results without creating runtime names', () => {
  const source = 'let length = $[](point) { point.x }; let answer = length(@{ let x = 1; });';
  const document = TextDocument.create('file:///example.pr', 'prim', 1, source);
  const stub = TextDocument.create(uri, 'prim-interface', 1, 'type Point = @{ let x: int; }; let length: $[](point: Point) -> int; let absent: int;');
  const docs = new Map([[uri, stub], [document.uri, document]]);
  const workspace = new HintWorkspace(docs);
  const context = workspace.context(document);
  const f = new Features(document, new Analysis(source, null, context));
  assert.deepEqual(f.diagnostics().filter(d => d.severity === 1), []);
  const member = f.definition(document.positionAt(source.indexOf('point.x') + 6));
  assert.equal(member.uri, uri);
  assert.match(f.hover(document.positionAt(source.indexOf('length'))).contents.value, /point: Point.*int/);
  assert.equal(f.analysis.symbols.find(s => s.name === 'answer').info.type, 'int');
  assert.ok(new Analysis('absent;', null, context).diagnostics.some(d => d.code === 'UndefinedName'));
  assert.equal(f.prepareRename(document.positionAt(source.indexOf('length'))), null);
  const invalid = new Analysis('let length = $[](x, y) { x };', null, context);
  assert.ok(invalid.diagnostics.some(d => d.code === 'InterfaceMismatch' && d.severity === 2));
  assert.equal(invalid.symbols.find(s => s.name === 'length').info.params.length, 2);
  const decorated = new Analysis('let wrap = $[](f) { f }; let length = @wrap $[](point) { point.x };', null, context);
  assert.equal(decorated.symbols.find(s => s.name === 'length').info.result.type, 'int');
  docs.set(uri, TextDocument.create(uri, 'prim-interface', 2, 'let length: $[](point: int) -> str;'));
  assert.equal(workspace.context(document).hints.get('length').info.result.type, 'str');
  docs.set(uri, TextDocument.create(uri, 'prim-interface', 3, 'let length: Missing;'));
  assert.equal(workspace.context(document).hints.size, 0);
});

test('workspace builtin overlays are validated and broken interfaces do not remove runtime builtins', () => {
  const root = path.resolve(__dirname, '../../..');
  const stdUri = pathToFileURL(path.join(root, 'builtin/std.pri')).href;
  const document = TextDocument.create(pathToFileURL(path.join(root, 'examples/hello_world.pr')).href, 'prim', 1, 'std.print(1);');
  const docs = new Map([[stdUri, TextDocument.create(stdUri, 'prim-interface', 1, 'let std: @{ let wrong: int; };')]]);
  const workspace = new HintWorkspace(docs); workspace.setRoots([pathToFileURL(root).href]);
  assert.ok(workspace.context(document).builtins.get('std').info.fields.has('print'));
  assert.ok(workspace.context(docs.get(stdUri)).analysis.diagnostics.some(d => d.code === 'BuiltinInterfaceMismatch'));
});

test('PRI TextMate scopes distinguish declaration names, parameters and type names', async () => {
  const vsctm = require('vscode-textmate');
  const oniguruma = require('vscode-oniguruma');
  const wasm = fs.readFileSync(require.resolve('vscode-oniguruma/release/onig.wasm'));
  await oniguruma.loadWASM(wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength));
  const registry = new vsctm.Registry({ onigLib: Promise.resolve({ createOnigScanner: s => new oniguruma.OnigScanner(s), createOnigString: s => new oniguruma.OnigString(s) }),
    loadGrammar: async scope => {
      const name = scope === 'source.prim-interface' ? 'pri' : 'prim';
      const file = path.resolve(__dirname, `../syntaxes/${name}.tmLanguage.json`);
      return vsctm.parseRawGrammar(fs.readFileSync(file, 'utf8'), file);
    } });
  const grammar = await registry.loadGrammar('source.prim-interface');
  const line = 'let move: $[](point: Point, step: int) -> unit;';
  const scopes = grammar.tokenizeLine(line, vsctm.INITIAL).tokens.map(t => ({ text: line.slice(t.startIndex, t.endIndex), scopes: t.scopes }));
  assert.ok(scopes.some(t => t.text === 'point' && t.scopes.includes('variable.parameter.prim')));
  assert.ok(scopes.some(t => t.text === 'Point' && t.scopes.includes('entity.name.type.prim')));
  assert.ok(scopes.some(t => t.text === 'int' && t.scopes.includes('support.type.prim')));
  registry.dispose();
});
