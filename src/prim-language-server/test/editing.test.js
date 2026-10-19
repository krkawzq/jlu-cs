'use strict';

const test = require('node:test');
const assert = require('node:assert/strict');
const { TextDocument } = require('vscode-languageserver-textdocument');
const { Analysis } = require('../server/analysis');
const { Features } = require('../server/features');

function fixture(text) {
  const document = TextDocument.create('file:///editing.pr', 'prim', 7, text);
  const features = new Features(document, new Analysis(text));
  const at = (word, nth = 0) => {
    let offset = -1;
    for (let i = 0; i <= nth; i++) offset = text.indexOf(word, offset + 1);
    assert.ok(offset >= 0, word);
    return document.positionAt(offset);
  };
  const range = { start: document.positionAt(0), end: document.positionAt(text.length) };
  return { document, features, at, range, analysis: features.analysis };
}
function apply(document, edit) {
  assert.equal(edit.documentChanges[0].textDocument.version, document.version);
  return TextDocument.applyEdits(document, edit.documentChanges[0].edits);
}
const codes = text => new Analysis(text).diagnostics.map(d => d.code);

test('type definition and type hints track constructors through object aliases', () => {
  const { features: f, at, range } = fixture('let Point = $[](x) { @{ let value = x; } }; let p = Point(1); let q = p; q.value;');
  assert.deepEqual(f.typeDefinition(at('q.value')).range.start, at('Point'));
  const hint = f.inlayHints(range).find(h => h.kind === 1 && h.label[1].value === 'Point');
  assert.deepEqual(hint.label[1].location.range.start, at('Point'));
  assert.ok(f.inlayHints(range).some(h => h.kind === 2 && h.label === 'x:'));
  assert.deepEqual(f.inlayHints(range, { types: false, parameters: false }), []);
  const narrow = { start: at('q.value'), end: at('q.value') };
  assert.deepEqual(f.inlayHints(narrow), []);
});

test('mutations invalidate the actual binding without affecting same-name locals or fields', () => {
  const { features: f, at } = fixture('let object = @{ let field = 1; }; { let object = 0; object = 2; }; object.field;');
  assert.deepEqual(f.definition(at('field', 1)).range.start, at('field'));
  const captured = fixture('let object = @{ let field = 1; }; let f = $[object]() { object = 2; }; object.field;');
  assert.deepEqual(captured.features.definition(captured.at('field', 1)).range.start, captured.at('field'));
  const shared = fixture('let object = @{ let field = 1; }; let f = $[&object]() { object = 2; }; object.field;');
  assert.equal(shared.features.definition(shared.at('field', 1)), null);
  const member = fixture('let o = @{ let f = $[](x) { x }; }; o.f = 1; o.f(2);');
  assert.equal(member.features.signatureHelp(member.at('2)')), null);
});

test('quick fixes add captures without choosing value or reference semantics for the user', () => {
  for (const captures of ['', 'other', 'other,', 'other /* note */', 'other, // note\n']) {
    const { features: f, document, range } = fixture(`let x = 1; let other = 0; let f = $[${captures}]() { x };`);
    const actions = f.codeActions(range).filter(a => a.diagnostics[0].code === 'MissingCapture');
    assert.equal(actions.length, 2);
    assert.ok(actions.every(a => !a.isPreferred));
    for (const action of actions) {
      const changed = apply(document, action.edit);
      assert.ok(!codes(changed).includes('MissingCapture'), changed);
      assert.ok(!new Analysis(changed).diagnostics.some(d => d.severity === 1), changed);
    }
  }
});

test('semicolon fixes insert before comments and retain subsequent declarations', () => {
  for (const text of ['let x = 1 // comment\nlet y = 2;', '@{ let x = 1; x }', 'let x = 1']) {
    const { features: f, document, range } = fixture(text);
    const action = f.codeActions(range).find(a => a.title === 'Add missing semicolon');
    assert.ok(action, text);
    const changed = apply(document, action.edit);
    assert.ok(!new Analysis(changed).diagnostics.some(d => d.severity === 1), changed);
    if (text.includes('//')) assert.ok(changed.includes('1; // comment'));
  }
  const incomplete = fixture('let f = $[]() { 1');
  assert.ok(!incomplete.features.codeActions(incomplete.range).some(a => a.title === 'Add missing semicolon'));
});

test('unused quick fix preserves initializer side effects and updates writes', () => {
  const { features: f, document, range } = fixture('let unused = std.print(1); unused = 2;');
  const action = f.codeActions(range).find(a => a.diagnostics[0].code === 'UnusedBinding');
  const changed = apply(document, action.edit);
  assert.equal(changed, 'let _unused = std.print(1); _unused = 2;');
  assert.deepEqual(f.codeActions(range, { only: ['refactor'] }), []);
  assert.deepEqual(f.codeActions(range, {}, false), []);
  assert.ok(!codes(changed).includes('UnusedBinding'));
});

test('rename permits independent scopes and fields but rejects capture of existing references', () => {
  const safe = fixture('let x = 1; let obj = @{ let y = 2; }; std.print(x); obj.y;');
  assert.doesNotThrow(() => safe.features.rename(safe.at('x'), 'y'));
  const nested = fixture('let x = 1; { let y = 2; std.print(y); }; std.print(x);');
  assert.doesNotThrow(() => nested.features.rename(nested.at('x'), 'y'));
  const unsafe = fixture('let x = 1; { let y = 2; std.print(x); };');
  assert.throws(() => unsafe.features.rename(unsafe.at('x'), 'y'), /conflict/);
  const capture = fixture('let x = 1; let f = $[x](y) { x + y };');
  assert.throws(() => capture.features.rename(capture.at('x'), 'y'), /conflict/);
  const unresolved = fixture('let x = 1; std.print(missing);');
  assert.throws(() => unresolved.features.rename(unresolved.at('x'), 'missing'), /resolution/);
});

test('completion replaces the whole word and avoids strings and comments', () => {
  const text = 'let longname = 1; longname; // longname\n"longname"';
  const { features: f, at, document } = fixture(text);
  const position = at('longname', 1); position.character += 3;
  const item = f.completion(position).items.find(i => i.label === 'longname');
  assert.equal(document.getText(item.textEdit.range), 'longname');
  const comment = at('longname', 2); comment.character += 2;
  assert.deepEqual(f.completion(comment).items, []);
  const string = at('longname', 3); string.character += 2;
  assert.deepEqual(f.completion(string).items, []);
  assert.equal(f.signatureHelp(string), null);
});

test('completion offers captures, callable decorators and negotiated snippets', () => {
  const capture = fixture('let outside = 1; let f = $[out');
  assert.ok(capture.features.completion(capture.range.end).items.some(i => i.label === 'outside'));
  const decorator = fixture('let plain = 1; let wrap = $[](f) { f }; @');
  const labels = decorator.features.completion(decorator.range.end).items.map(i => i.label);
  assert.ok(labels.includes('wrap')); assert.ok(labels.includes('jit')); assert.ok(!labels.includes('plain')); assert.ok(!labels.includes('let'));
  const snippets = fixture('cl');
  assert.ok(snippets.features.completion(snippets.range.end, true).items.some(i => i.kind === 15 && i.insertTextFormat === 2));
  assert.ok(snippets.features.completion(snippets.range.end, false).items.every(i => i.kind !== 15));
  const unfinished = fixture('let f = $[](parameter) {\n    ');
  const unfinishedLabels = unfinished.features.completion(unfinished.range.end).items.map(i => i.label);
  assert.ok(unfinishedLabels.includes('parameter'));
  assert.ok(!unfinishedLabels.includes('f'));
});

test('hierarchical symbols, selection expansion and comment folding retain source containment', () => {
  const { features: f, at, document } = fixture('/* block\n comment */\nlet Outer = $[](param) {\n let inner = $[](x) { x };\n @{ let field = param; }\n};');
  const outer = f.documentSymbols().find(s => s.name === 'Outer');
  assert.ok(outer.children.some(s => s.name === 'param'));
  assert.ok(outer.children.find(s => s.name === 'inner').children.some(s => s.name === 'x'));
  assert.ok(f.foldingRanges().some(r => r.kind === 'comment' && r.startLine === 0 && r.endLine === 1));
  let selection = f.selectionRanges([at('field')])[0], levels = 0;
  while (selection.parent) {
    assert.ok(document.offsetAt(selection.parent.range.start) <= document.offsetAt(selection.range.start));
    assert.ok(document.offsetAt(selection.parent.range.end) >= document.offsetAt(selection.range.end));
    selection = selection.parent; levels++;
  }
  assert.ok(levels >= 3);
});

test('known call warnings distinguish argument count and reference mode from unknown dynamic calls', () => {
  assert.ok(codes('let f = $[](x) { x }; f();').includes('CallArity'));
  assert.ok(codes('let f = $[](&x) { x }; f(1);').includes('ArgumentMode'));
  assert.ok(codes('std.typo(1);').includes('UnknownField'));
  assert.ok(!codes('let f = $[](g) { g(1, 2); };').includes('CallArity'));
  assert.ok(!codes('let f = $[](x) { x }; f(').includes('CallArity'));
});

test('unfinished calls retain signatures, scopes and later declarations', () => {
  const { features: f, at, analysis } = fixture('let f = $[](x, y) { x }; let result = f(1, ; let good = 2; std.print(good);');
  assert.deepEqual(f.definition(at('good', 1)).range.start, at('good'));
  assert.equal(f.signatureHelp(at('; let good')).activeParameter, 1);
  assert.ok(analysis.diagnostics.some(d => d.code === 'ExpectedToken'));
  assert.doesNotThrow(() => new Analysis('@x '.repeat(200)));
  assert.ok(codes('1' + '+1'.repeat(3000)).includes('AnalysisLimit'));
});
