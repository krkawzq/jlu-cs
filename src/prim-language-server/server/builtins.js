'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { createInterfaces } = require('./interfaces');
const { entries } = require('./builtin-signatures.json');
let defaultEnvironment;

function defaultBuiltinDirectory() {
  const bundled = path.resolve(__dirname, '../builtin');
  if (fs.existsSync(path.join(bundled, 'std.pri'))) return bundled;
  const development = path.resolve(__dirname, '../../../builtin');
  return development;
}

function loadBuiltinSources(directory = defaultBuiltinDirectory(), overlays = new Map()) {
  return fs.readdirSync(directory).filter(name => name.endsWith('.pri')).sort().map(name => {
    const file = path.join(directory, name), uri = pathToFileURL(file).href;
    const document = overlays.get(uri);
    return { uri, text: document ? document.getText() : fs.readFileSync(file, 'utf8'), version: document?.version };
  });
}

function validateBuiltins(environment) {
  const actual = new Map();
  const errors = [];
  function visit(symbol, name) {
    if (symbol.info.callable) {
      actual.set(name, symbol.info.params?.length);
      if (symbol.info.params?.some(p => p.byRef)) errors.push(`${name}: prim-rs builtins accept only value parameters`);
    }
    else if (symbol.info.fields) for (const field of symbol.info.fields.values()) visit(field, `${name}.${field.name}`);
  }
  for (const [name, symbol] of environment.values) visit(symbol, name);
  const expected = new Map(entries.map(e => [e.name, e.arity]));
  for (const [name, arity] of expected) if (actual.get(name) !== arity) errors.push(`${name}: prim-rs declares ${arity} parameters; .pri declares ${actual.get(name) ?? 'none'}`);
  for (const name of actual.keys()) if (!expected.has(name)) errors.push(`${name}: function is not registered in prim-rs`);
  for (const analysis of environment.analyses.values()) for (const diagnostic of analysis.diagnostics) if (diagnostic.severity === 1) errors.push(`${analysis.uri}: ${diagnostic.message}`);
  return errors;
}

function createBuiltins(environment) {
  environment ||= (defaultEnvironment ||= createInterfaces(loadBuiltinSources()));
  function mark(symbol, qualifiedName) {
    const info = { ...symbol.info };
    if (info.fields) info.fields = new Map([...info.fields].map(([name, field]) => [name, mark(field, `${qualifiedName}.${name}`)]));
    return { ...symbol, info, qualifiedName, builtin: true, readonly: true,
      kind: info.callable ? 'function' : info.fields ? 'namespace' : symbol.kind };
  }
  return new Map([...environment.values].filter(([name]) => ['std', 'jit'].includes(name)).map(([name, symbol]) => [name, mark(symbol, name)]));
}

module.exports = { createBuiltins, loadBuiltinSources, defaultBuiltinDirectory, validateBuiltins };
