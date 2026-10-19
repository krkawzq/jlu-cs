'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const { Analysis } = require('../server/analysis');
const { lex, Parser } = require('../server/syntax');

function normalize(e) {
  if (!e) return null;
  const arg = a => [a.byRef ? 'ref' : 'value', normalize(a.expression)];
  const bindings = list => list.map(b => [b.name.text, b.byRef]);
  switch (e.kind) {
    case 'body': return ['body', e.statements.map(normalize), normalize(e.tail)];
    case 'let': return ['let', e.name.text, arg(e.value)];
    case 'assign': return ['assign', normalize(e.target), normalize(e.value)];
    case 'expression': case 'return': case 'break': return [e.kind, normalize(e.value)];
    case 'int': case 'float': case 'name': return [e.kind, e.token.text];
    case 'bool': return ['bool', e.token.text === 'true'];
    case 'str': case 'unit': case 'self': case 'continue': return [e.kind];
    case 'group': return ['group', normalize(e.value)];
    case 'unary': return ['unary', e.op.text, normalize(e.value)];
    case 'binary': return ['binary', e.op.text, normalize(e.left), normalize(e.right)];
    case 'call': return ['call', normalize(e.callee), e.args.map(arg)];
    case 'field': return ['field', normalize(e.receiver), e.name.text];
    case 'block': case 'data': case 'loop': return [e.kind, normalize(e.body)];
    case 'if': return ['if', normalize(e.condition), normalize(e.body), normalize(e.other)];
    case 'closure': return ['closure', bindings(e.captures), bindings(e.params), normalize(e.body)];
    case 'decorate': return ['decorate', e.name.text, normalize(e.target)];
    default: throw new Error(`unexpected AST node: ${e.kind}`);
  }
}

function cases() {
  const root = path.resolve(__dirname, '../..');
  const samples = new Set([
    '$check_guess(guess, target) { guess == target };',
    '$check_guess(guess: i32, target: i32): str { "ok" };',
    'let f = $[](x) { x }; let value = if true { f } else if false { f } else { f }(2);',
    'let f = if true { 1 } else if false { 2 } else { 3 } + 4;',
    'std.print = 1;', 'let s = std; s.print = 1;', 'let alias = &std.print;',
    'let f = $[&std]() {};', 'let f = $[jit]() {};',
    '"raw\tcontrol"', '"raw\u007fcontrol"', '\uFEFF1', 'let emoji = "😀";\r\nunknown;',
    '('.repeat(180) + '1' + ')'.repeat(180),
    '('.repeat(257) + '1' + ')'.repeat(257),
    '$[]() {'.repeat(100) + '1' + '}'.repeat(100),
    '1' + '+1'.repeat(400),
  ]);
  for (const folder of ['prim-rs/examples', '../examples']) {
    for (const name of fs.readdirSync(path.join(root, folder)).filter(n => n.endsWith('.pr'))) samples.add(fs.readFileSync(path.join(root, folder, name), 'utf8'));
  }
  const tests = fs.readFileSync(path.join(root, 'prim-rs/tests/language.rs'), 'utf8');
  for (const match of tests.matchAll(/r(#+)"([\s\S]*?)"\1|"(?:\\.|[^"\\])*"/g)) {
    if (match[1]) samples.add(match[2]);
    else {
      const json = match[0].replace(/\\u\{([\da-fA-F]+)\}/g, (_, digits) => String.fromCodePoint(parseInt(digits, 16))).replace(/\\0/g, '\\u0000');
      try { samples.add(JSON.parse(json)); } catch { /* Rust format literals may not be standalone source. */ }
    }
  }
  const operators = ['+', '-', '*', '/', '%', '==', '!=', '<', '<=', '>', '>=', '&&', '||'];
  for (const a of operators) for (const b of operators) samples.add(`1 ${a} 2 ${b} 3`);
  for (const suffix of ['(1)', '.x', ' + 2', ' == 2', '(1).x']) {
    samples.add(`if true { f } else if false { g } else { h }${suffix}`);
    samples.add(`if true { f } else { g }${suffix}`);
  }
  return [...samples];
}

test('actual prim-rs frontend and language server agree on acceptance and AST structure', {
  skip: !process.env.PRIM_FRONTEND_PROBE, timeout: 30000
}, t => {
  const samples = cases();
  const input = Buffer.concat(samples.flatMap(source => {
    const text = Buffer.from(source), length = Buffer.alloc(4); length.writeUInt32LE(text.length);
    return [length, text];
  }));
  const result = spawnSync(process.env.PRIM_FRONTEND_PROBE, [], { input, timeout: 20000, maxBuffer: 24 * 1024 * 1024 });
  assert.equal(result.status, 0, result.stderr?.toString());
  const responses = result.stdout.toString().trim().split('\n').map(JSON.parse);
  assert.equal(responses.length, samples.length);
  const failures = [];
  let trees = 0;
  samples.forEach((source, i) => {
    const rust = responses[i];
    const diagnostics = [];
    const ast = new Parser(lex(source, (...d) => diagnostics.push(d)), (...d) => diagnostics.push(d)).body();
    const analysis = new Analysis(source);
    const errors = analysis.diagnostics.filter(d => d.severity === 1);
    if (rust.ok !== (errors.length === 0)) failures.push({ source, rust: { ok: rust.ok, phase: rust.phase, code: rust.code }, lsp: errors.map(d => d.code) });
    if (rust.tree && diagnostics.length === 0) {
      trees++;
      try { assert.deepEqual(normalize(ast), rust.tree); }
      catch { failures.push({ source, mismatch: 'AST', rust: rust.tree, lsp: normalize(ast) }); }
    }
  });
  t.diagnostic(`${samples.length} source cases; ${trees} AST comparisons against the compiled prim-rs modules`);
  assert.deepEqual(failures, []);
});
