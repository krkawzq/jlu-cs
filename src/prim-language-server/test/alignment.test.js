'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { TextDocument } = require('vscode-languageserver-textdocument');
const { Analysis } = require('../server/analysis');
const { Features } = require('../server/features');
const { extractBuiltins } = require('../../../scripts/prim-language-server/sync-builtins');
const catalog = require('../server/builtin-signatures.json');

function features(source) {
  const document = TextDocument.create('file:///alignment.pr', 'prim', 1, source);
  return new Features(document, new Analysis(source));
}

test('standard library completion and signatures match every current prim-rs registry entry', () => {
  const source = fs.readFileSync(path.resolve(__dirname, '../../prim-rs/src/builtins.rs'), 'utf8');
  assert.deepEqual(catalog, extractBuiltins(source));
  for (const { name, arity } of catalog.entries) {
    const f = features(`${name}(${Array.from({ length: arity }, () => '1').join(', ')})`);
    assert.ok(!f.diagnostics().some(d => ['UndefinedName', 'UnknownField', 'CallArity'].includes(d.code)), name);
    const signature = f.signatureHelp(f.document.positionAt(name.length + 1));
    assert.equal(signature.signatures[0].parameters.length, arity, name);
    const prefix = name.includes('.') ? name.slice(0, name.lastIndexOf('.') + 1) : '';
    const completion = features(prefix);
    assert.ok(completion.completion(completion.document.positionAt(prefix.length)).items.some(i => i.label === name.split('.').at(-1)), name);
  }
  assert.match(features('std.len').hover({ line: 0, character: 5 }).contents.value, /fields in a closure/);
});

test('complete conditional is the receiver of postfix calls, fields and binary operations', () => {
  for (const [suffix, kind] of [['(1)', 'call'], ['.field', 'field'], [' + 1', 'binary']]) {
    const f = features(`let f = $[](x) { x }; if true { f } else if false { f } else { f }${suffix}`);
    const tail = f.analysis.ast.tail;
    assert.equal(tail.kind, kind);
    assert.equal((tail.callee || tail.receiver || tail.left).kind, 'if');
  }
  const f = features('let inc = $[](n) { n + 1 }; let x = if true { inc } else if false { inc } else { inc }(2);');
  assert.equal(f.analysis.calls.at(-1).info.params[0].name.text, 'n');
});

test('field-slot errors stay runtime warnings while readonly bindings are static errors', () => {
  for (const source of ['std.print = 1;', 'let s = std; s.print = 1;', 'let alias = &std.print;', 'std.term.move = 0;']) {
    const diagnostics = features(source).diagnostics();
    assert.ok(!diagnostics.some(d => d.severity === 1), source);
    assert.ok(diagnostics.some(d => d.code === 'ReadOnlySlot' && d.severity === 2), source);
  }
  assert.ok(features('std = 1;').diagnostics().some(d => d.code === 'InvalidPlace' && d.severity === 1));
  for (const name of ['std', 'jit']) for (const mode of ['', '&']) {
    assert.ok(features(`let f = $[${mode}${name}]() {};`).diagnostics().some(d => d.code === 'ReservedBinding'));
  }
});

test('named legacy functions explain the accepted prim-rs syntax without accepting annotations', () => {
  const diagnostic = features('$check_guess(guess, target) { true };').diagnostics().find(d => d.code === 'SyntaxError');
  assert.match(diagnostic.message, /let check_guess = \$\[\]/);
  assert.ok(features('let check_guess = $[](guess: i32) { true };').diagnostics().some(d => d.code === 'IllegalCharacter'));
});

test('root examples use executable Core 1 syntax and resolve check_guess and std array access', () => {
  const root = path.resolve(__dirname, '../../../examples');
  for (const file of fs.readdirSync(root).filter(n => n.endsWith('.pr'))) {
    const f = features(fs.readFileSync(path.join(root, file), 'utf8'));
    assert.deepEqual(f.diagnostics().filter(d => d.severity === 1 || d.code === 'UnknownField'), [], file);
    if (file === 'guess_number.pr') {
      const source = f.document.getText();
      const position = f.document.positionAt(source.lastIndexOf('check_guess('));
      assert.equal(f.document.offsetAt(f.definition(position).range.start), source.indexOf('check_guess'));
    }
  }
});
