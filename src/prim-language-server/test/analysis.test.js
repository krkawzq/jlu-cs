'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { TextDocument } = require('vscode-languageserver-textdocument');
const { Analysis } = require('../server/analysis');
const { Features, TOKEN_TYPES } = require('../server/features');

function fixture(text) {
  const document = TextDocument.create('file:///test.pr', 'prim', 3, text);
  const features = new Features(document, new Analysis(text));
  const at = (word, nth = 0) => {
    let offset = -1;
    for (let i = 0; i <= nth; i++) offset = text.indexOf(word, offset + 1);
    assert.notEqual(offset, -1, `missing ${word}`);
    return document.positionAt(offset);
  };
  return { features, document, at, analysis: features.analysis };
}
const errors = source => new Analysis(source).diagnostics.filter(d => d.severity === 1).map(d => d.code);

test('Core 1 examples parse and resolve without errors', () => {
  const folder = path.join(__dirname, '../../prim-rs/examples');
  for (const name of fs.readdirSync(folder).filter(n => n.endsWith('.pr') && !n.startsWith('error.'))) {
    assert.deepEqual(errors(fs.readFileSync(path.join(folder, name), 'utf8')), [], name);
  }
  assert.deepEqual(errors(fs.readFileSync(path.join(__dirname, 'sample.pr'), 'utf8')), []);
});

test('definition follows lexical scope, capture sources and object fields', () => {
  const { features: f, at, document } = fixture('let x = 1; { let x = 2; std.print(x); }; let f = $[x]() { x }; let o = @{ let x = 3; }; o.x;');
  assert.deepEqual(f.definition(at('x', 2)).range.start, at('x', 1));
  assert.deepEqual(f.definition(at('x', 3)).range.start, at('x', 0));
  assert.deepEqual(f.definition(at('x', 4)).range.start, at('x', 3));
  assert.deepEqual(f.definition(at('x', 6)).range.start, at('x', 5));
  const edits = f.rename(at('x', 0), 'outer').documentChanges[0];
  assert.equal(edits.textDocument.version, document.version);
  assert.equal(edits.edits.length, 3);
  assert.equal(f.references(at('x', 0), false).length, 2);
  assert.throws(() => f.rename(at('x', 0), 'o'), /conflict/);
  assert.throws(() => f.rename(at('x', 0), 'std'), /identifier/);
  assert.equal(f.prepareRename(at('std')), null);
});

test('constructor types, functions, variables, properties and parameters have distinct semantic tokens', () => {
  const text = 'let Point = $[](x) { @{ let value = x; } }; let p = Point(1); p.value;';
  const { features: f, at, document } = fixture(text);
  const tokens = new Map(); let line = 0, character = 0;
  const data = f.semanticTokens().data;
  for (let i = 0; i < data.length; i += 5) {
    line += data[i]; character = data[i] ? data[i + 1] : character + data[i + 1];
    tokens.set(document.offsetAt({ line, character }), TOKEN_TYPES[data[i + 3]]);
  }
  assert.equal(tokens.get(document.offsetAt(at('Point'))), 'type');
  assert.equal(tokens.get(document.offsetAt(at('Point', 1))), 'type');
  assert.equal(tokens.get(document.offsetAt(at('p ='))), 'variable');
  assert.equal(tokens.get(document.offsetAt(at('x)'))), 'parameter');
  assert.equal(tokens.get(document.offsetAt(at('value', 1))), 'property');
  assert.deepEqual(f.definition(at('value', 1)).range.start, at('value'));
  assert.match(f.hover(at('Point')).contents.value, /Uppercase constructor/);
  const ordinary = fixture('let Number = 1; let Fn = $[]() { 1 };');
  assert.equal(ordinary.analysis.symbols.find(s => s.name === 'Number').kind, 'variable');
  assert.equal(ordinary.analysis.symbols.find(s => s.name === 'Fn').kind, 'function');
});

test('completion respects closure visibility and offers inferred members', () => {
  const scope = fixture('let outer = 1; let f = $[](param) { pa };');
  const names = scope.features.completion(scope.at('pa }')).items.map(i => i.label);
  assert.ok(names.includes('param')); assert.ok(!names.includes('outer'));
  const object = fixture('let Point = $[](x) { @{ let value = x; } }; let p = Point(1); p.');
  const members = object.features.completion(object.document.positionAt(object.document.getText().length)).items;
  assert.deepEqual(members.map(i => i.label), ['value']);
  const std = fixture('std.');
  assert.ok(std.features.completion({ line: 0, character: 4 }).items.some(i => i.label === 'print'));
});

test('signatures include reference modes and active arguments, including unfinished calls', () => {
  const sample = fixture('let f = $[](&x, y) { x }; let a = 1; f(&a, 2);');
  const help = sample.features.signatureHelp(sample.at('2)'));
  assert.equal(help.signatures[0].label, 'f(&x, y)'); assert.equal(help.activeParameter, 1);
  const incomplete = fixture('let f = $[](x, y) { x }; f(1,');
  assert.equal(incomplete.features.signatureHelp(incomplete.document.positionAt(incomplete.document.getText().length)).activeParameter, 1);
  const builtin = fixture('std.print(');
  assert.equal(builtin.features.signatureHelp({ line: 0, character: 10 }).signatures[0].label, 'std.print(value: any) -> unit');
});

test('static errors cover lexical, syntax, capture, binding, place and control boundaries', () => {
  for (const [source, code] of [
    ['let x = 0b102;', 'InvalidNumber'], ['let x = "\\u{D800}";', 'InvalidEscape'],
    ['/* unfinished', 'UnterminatedComment'], ['let x = "unterminated', 'UnterminatedString'],
    ['let x = 1 let y = 2;', 'MissingSemicolon'], ['@{ 1 }', 'DataClosureTail'],
    ['1 < 2 < 3', 'ChainedComparison'], ['let x = 1; let f = $[]() { x };', 'MissingCapture'],
    ['unknown;', 'UndefinedName'], ['let x = 1; let x = 2;', 'DuplicateBinding'],
    ['let f = $[](x) { let x = 2; };', 'DuplicateBinding'], ['let std = 1;', 'ReservedBinding'],
    ['let x = 1; let f = $[x, &x]() {};', 'DuplicateBinding'],
    ['std = 1;', 'InvalidPlace'], ['let x = &(1 + 2);', 'InvalidPlace'],
    ['return 1;', 'InvalidControlTarget'], ['loop { let f = $[]() { break; }; };', 'InvalidControlTarget'],
    ['let f = $[]() { @{ return 1; } };', 'InvalidControlTarget'], ['self;', 'InvalidSelf'],
    ['9223372036854775808', 'IntegerLiteralOutOfRange'], ['1e999', 'FloatLiteralOutOfRange']
  ]) assert.ok(errors(source).includes(code), `${source}: expected ${code}, got ${errors(source)}`);
  for (const source of ['-(9223372036854775808)', 'let f = $[]() { @{ self; } };', 'let x = 1; { let x = x; x }', '0x1e+2', 'let s = "café😀"; std.print(s);']) {
    assert.deepEqual(errors(source), [], source);
  }
});

test('warnings distinguish unused reads, exposed fields, captures, shadowing and unreachable code', () => {
  const f = fixture('let x = 1; x = 2; let object = @{ let field = 1; }; let g = $[]() { return 1; 2 }; { let x = 3; x };').features;
  const codes = f.diagnostics().map(d => d.code);
  for (const code of ['UnusedBinding', 'UnreachableCode', 'ShadowedBinding']) assert.ok(codes.includes(code));
  assert.ok(!f.diagnostics().some(d => d.code === 'UnusedBinding' && d.message.startsWith('field ')));
  assert.deepEqual(f.diagnostics(false), []);
  const captured = fixture('let x = 1; let f = $[x]() { x }; f();');
  assert.deepEqual(captured.features.diagnostics(), []);
});

test('UTF-16 and CRLF positions survive non-ASCII strings', () => {
  const sample = fixture('let s = "😀"; let x = 1;\r\nstd.print(x);');
  assert.deepEqual(sample.features.definition(sample.at('x', 1)).range.start, { line: 0, character: 18 });
  assert.equal(sample.features.references(sample.at('x'), true)[1].range.start.line, 1);
});

test('dynamic values and decorators do not invent fields or static type errors', () => {
  const sample = fixture('let f = $[](x) { x.field }; let obj = @{ let value = 1; }; obj = 1; obj.value;');
  assert.deepEqual(sample.analysis.diagnostics.filter(d => d.severity === 1), []);
  assert.equal(sample.features.definition(sample.at('value', 1)), null);
  const decorated = fixture('let wrap = $[](f) { f }; let Point = @wrap $[]() { @{ let x = 1; } };');
  assert.equal(decorated.analysis.symbols.find(s => s.name === 'Point').kind, 'function');
});

test('malformed editing inputs terminate and retain later declarations', () => {
  for (const input of ['}', ')', '$[', 'let f = $[](x', 'f(,)', 'std..', 'if {', '@', '('.repeat(300)]) assert.doesNotThrow(() => new Analysis(input));
  const sample = fixture('let = 1; let good = 2; std.print(good);');
  assert.deepEqual(sample.features.definition(sample.at('good', 1)).range.start, sample.at('good'));
});
