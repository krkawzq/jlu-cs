const fs = require('fs');
const path = require('path');
const vsctm = require('vscode-textmate');
const oniguruma = require('vscode-oniguruma');

const REQUIRED_SCOPES = [
  'comment.line.double-slash.prim',
  'comment.block.prim',
  'string.quoted.double.prim',
  'constant.character.escape.prim',
  'constant.character.escape.unicode.prim',
  'constant.numeric.integer.prim',
  'constant.numeric.hex.prim',
  'constant.numeric.octal.prim',
  'constant.numeric.binary.prim',
  'constant.numeric.float.prim',
  'keyword.other.let.prim',
  'keyword.control.prim',
  'constant.language.boolean.prim',
  'constant.language.unit.prim',
  'keyword.operator.closure.prim',
  'keyword.operator.reference.prim',
  'keyword.operator.decorator.prim',
  'keyword.operator.data-closure.prim',
  'variable.parameter.capture.prim',
  'variable.parameter.prim',
  'entity.name.function.call.prim',
  'entity.name.function.decorator.prim',
  'variable.other.property.prim',
  'variable.language.self.prim',
  'support.variable.std.prim',
  'support.function.builtin.prim',
  'variable.other.declaration.prim',
  'punctuation.accessor.prim',
  'punctuation.terminator.statement.prim'
];

async function main() {
  const root = path.resolve(__dirname, '..');
  const grammarPath = path.join(root, 'syntaxes', 'prim.tmLanguage.json');
  const samplePath = process.argv[2]
    ? path.resolve(process.argv[2])
    : path.join(__dirname, 'sample.pr');

  const wasm = fs.readFileSync(require.resolve('vscode-oniguruma/release/onig.wasm'));
  await oniguruma.loadWASM(
    wasm.buffer.slice(wasm.byteOffset, wasm.byteOffset + wasm.byteLength)
  );

  const registry = new vsctm.Registry({
    onigLib: Promise.resolve({
      createOnigScanner: (sources) => new oniguruma.OnigScanner(sources),
      createOnigString: (s) => new oniguruma.OnigString(s)
    }),
    loadGrammar: async (scopeName) => {
      if (scopeName !== 'source.prim') return null;
      return vsctm.parseRawGrammar(fs.readFileSync(grammarPath, 'utf8'), grammarPath);
    }
  });

  const grammar = await registry.loadGrammar('source.prim');
  if (!grammar) throw new Error('Unable to load source.prim grammar');

  const lines = fs.readFileSync(samplePath, 'utf8').split(/\r?\n/);
  let ruleStack = vsctm.INITIAL;
  const seen = new Set();
  let tokenCount = 0;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const result = grammar.tokenizeLine(line, ruleStack);
    ruleStack = result.ruleStack;
    console.log(`${String(i + 1).padStart(3)} | ${line}`);
    for (const token of result.tokens) {
      const text = line.slice(token.startIndex, token.endIndex);
      token.scopes.forEach((scope) => seen.add(scope));
      tokenCount++;
      if (text.trim().length > 0) {
        console.log(`      ${JSON.stringify(text)}  ${token.scopes.join(' ')}`);
      }
    }
  }

  const missing = REQUIRED_SCOPES.filter((scope) => !seen.has(scope));
  console.log(`\n${tokenCount} tokens, ${seen.size} distinct scopes`);
  if (missing.length > 0) {
    console.error(`Missing expected scopes:\n  ${missing.join('\n  ')}`);
    process.exitCode = 1;
  } else {
    console.log('Scope check passed.');
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
