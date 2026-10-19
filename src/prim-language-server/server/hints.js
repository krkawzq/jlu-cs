'use strict';

const fs = require('node:fs');
const path = require('node:path');
const { fileURLToPath, pathToFileURL } = require('node:url');
const { createInterfaces } = require('./interfaces');
const { loadBuiltinSources, defaultBuiltinDirectory, createBuiltins, validateBuiltins } = require('./builtins');

function isInterface(document) {
  return document.languageId === 'prim-interface' || /\.pri$/i.test(document.uri);
}
function filePath(uri) {
  try { return uri.startsWith('file:') ? fileURLToPath(uri) : null; } catch { return null; }
}

class HintWorkspace {
  constructor(documents) {
    this.documents = documents;
    this.roots = [];
    this.defaults = createInterfaces(loadBuiltinSources());
  }
  setRoots(uris) { this.roots = uris.map(filePath).filter(Boolean).sort((a, b) => b.length - a.length); }
  context(document) {
    const file = filePath(document.uri);
    const root = file && this.roots.find(dir => { const relative = path.relative(dir, file); return !relative.startsWith('..' + path.sep) && relative !== '..' && !path.isAbsolute(relative); });
    const localBuiltin = root && path.join(root, 'builtin');
    const directory = localBuiltin && fs.existsSync(path.join(localBuiltin, 'std.pri')) ? localBuiltin : defaultBuiltinDirectory();
    const builtinSources = loadBuiltinSources(directory, this.documents);
    const builtinEnvironment = createInterfaces(builtinSources);
    const builtinErrors = validateBuiltins(builtinEnvironment);
    let siblingUri;
    if (isInterface(document)) siblingUri = document.uri;
    else if (file?.endsWith('.pr')) siblingUri = pathToFileURL(file.slice(0, -3) + '.pri').href;
    const sources = [...builtinSources];
    if (siblingUri && !sources.some(s => s.uri === siblingUri)) {
      const open = this.documents.get(siblingUri), filename = filePath(siblingUri);
      if (open) sources.push({ uri: siblingUri, text: open.getText(), version: open.version });
      else if (filename && fs.existsSync(filename)) sources.push({ uri: siblingUri, text: fs.readFileSync(filename, 'utf8') });
    }
    const environment = createInterfaces(sources);
    if (builtinErrors.length) {
      for (const analysis of environment.analyses.values()) {
        if (builtinSources.some(s => s.uri === analysis.uri)) analysis.report('BuiltinInterfaceMismatch',
          `Standard library interfaces are out of sync with prim-rs; using the bundled copy for hints. ${builtinErrors.slice(0, 3).join('; ')}`, { start: 0, end: 0 });
      }
    }
    const sibling = environment.analyses.get(siblingUri);
    const validHints = sibling && !sibling.diagnostics.some(d => d.severity === 1);
    const hints = new Map(validHints ? [...environment.values].filter(([, symbol]) => symbol.location.uri === siblingUri && !['std', 'jit'].includes(symbol.name)) : []);
    return { analysis: environment.analyses.get(document.uri),
      builtins: createBuiltins(builtinErrors.length ? this.defaults : builtinEnvironment), hints };
  }
}

module.exports = { HintWorkspace, isInterface };
