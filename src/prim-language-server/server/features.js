'use strict';

const { KEYWORDS } = require('./syntax');
const { Analysis, rootSymbol, signature } = require('./analysis');
const { typeLabel } = require('./interfaces');
const TOKEN_TYPES = ['type', 'function', 'parameter', 'variable', 'property', 'namespace'];
const TOKEN_MODIFIERS = ['declaration', 'readonly', 'defaultLibrary'];
const SYMBOL_KINDS = { type: 5, function: 12, parameter: 13, variable: 13, property: 8, namespace: 3 };
const COMPLETION_KINDS = { type: 7, function: 3, parameter: 6, variable: 6, property: 10, namespace: 9 };

class Features {
  constructor(document, analysis) { this.document = document; this.analysis = analysis; }
  range(span) { return { start: this.document.positionAt(span.start), end: this.document.positionAt(span.end) }; }
  location(span) { return { uri: this.document.uri, range: this.range(span) }; }
  at(position) { return this.analysis.occurrenceAt(this.document.offsetAt(position)); }
  diagnostics(warnings = true) {
    return this.analysis.diagnostics.filter(d => warnings || d.severity !== 2).map(d => ({
      range: this.range(d), severity: d.severity, code: d.code, source: 'prim', message: d.message,
      ...(d.tags ? { tags: d.tags } : {}), ...(d.data ? { data: d.data } : {})
    }));
  }
  definition(position) {
    const occurrence = this.at(position);
    const symbol = occurrence?.declaration && occurrence.symbol?.source ? occurrence.symbol.source : occurrence?.symbol;
    return symbol?.location || (symbol?.token ? this.location(symbol.token) : null);
  }
  typeDefinition(position) {
    const symbol = this.at(position)?.symbol;
    const type = symbol?.kind === 'type' ? symbol : this.analysis.value(symbol).constructorSymbol;
    return type?.location || (type?.token ? this.location(type.token) : null) || this.analysis.value(symbol).typeLocation || null;
  }
  references(position, includeDeclaration) {
    const symbol = this.at(position)?.symbol;
    if (!symbol || symbol.builtin) return [];
    return this.analysis.references(symbol).filter(o => includeDeclaration || !o.declaration || o.symbol.source)
      .map(o => this.location(o.token));
  }
  highlights(position) {
    const symbol = this.at(position)?.symbol;
    if (!symbol) return [];
    return this.analysis.references(symbol).map(o => ({ range: this.range(o.token), kind: o.write || o.declaration ? 3 : 2 }));
  }
  hover(position) {
    const occurrence = this.at(position), symbol = occurrence?.symbol;
    if (!symbol) return null;
    const description = symbol.kind === 'type' ? (symbol.interfaceDeclaration ? '\n\n.pri type declaration; does not create a runtime binding.' : '\n\nUppercase constructor returning a data closure, displayed as a type; it remains an ordinary callable closure in Prim.') : '';
    const fields = symbol.info.fields ? `\n\nFields: ${[...symbol.info.fields.keys()].map(n => `\`${n}\``).join(', ') || 'none'}` : '';
    return { range: this.range(occurrence.token), contents: { kind: 'markdown',
      value: `\`\`\`prim\n${signature(symbol)}\n\`\`\`${description}${symbol.doc ? `\n\n${symbol.doc}` : ''}${fields}\n\nType information is a static approximation; runtime values remain dynamically typed.` } };
  }
  prepareRename(position) {
    const occurrence = this.at(position);
    if (this.analysis.isInterface || !occurrence?.symbol?.token || occurrence.symbol.external || occurrence.symbol.hintLocation || rootSymbol(occurrence.symbol).builtin) return null;
    return { range: this.range(occurrence.token), placeholder: occurrence.symbol.name };
  }
  rename(position, newName) {
    const occurrence = this.at(position);
    if (!this.prepareRename(position)) throw new Error('Only resolved user bindings can be renamed.');
    if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(newName) || KEYWORDS.has(newName) || ['std', 'jit'].includes(newName)) {
      throw new Error('The new name must be a Prim identifier that is not a keyword or reserved name.');
    }
    const refs = this.analysis.references(occurrence.symbol);
    if (newName === occurrence.symbol.name) return { documentChanges: [] };
    const sorted = [...refs].sort((a, b) => a.token.start - b.token.start);
    let candidate = this.document.getText();
    for (const { token } of [...sorted].reverse()) candidate = candidate.slice(0, token.start) + newName + candidate.slice(token.end);
    const offsets = []; let delta = 0;
    for (const { token } of sorted) { delta += newName.length - (token.end - token.start); offsets.push({ start: token.start, delta }); }
    const shifted = offset => {
      let lo = 0, hi = offsets.length;
      while (lo < hi) { const mid = (lo + hi) >>> 1; if (offsets[mid].start < offset) lo = mid + 1; else hi = mid; }
      return offset + (lo ? offsets[lo - 1].delta : 0);
    };
    const next = new Analysis(candidate, null, this.analysis.options);
    const nextOccurrences = new Map(next.occurrences.map(o => [o.token.start, o]));
    const identity = (symbol, transform) => symbol?.external ? `${symbol.location?.uri}:${symbol.token.start}` : symbol?.token ? transform(symbol.token.start) : symbol?.builtin ? `builtin:${symbol.qualifiedName || symbol.name}` : null;
    // A rename must preserve resolution of every occurrence, including names that
    // would otherwise become accidentally captured by the renamed declaration.
    for (const old of this.analysis.occurrences) {
      const updated = nextOccurrences.get(shifted(old.token.start));
      if (!updated || identity(old.symbol, shifted) !== identity(updated.symbol, n => n)
          || identity(rootSymbol(old.symbol), shifted) !== identity(rootSymbol(updated.symbol), n => n)) {
        throw new Error(`${newName} would conflict or change name resolution.`);
      }
    }
    if (next.diagnostics.filter(d => d.code === 'DuplicateBinding').length > this.analysis.diagnostics.filter(d => d.code === 'DuplicateBinding').length) {
      throw new Error(`${newName} would conflict with an existing declaration.`);
    }
    return { documentChanges: [{ textDocument: { uri: this.document.uri, version: this.document.version },
      edits: refs.map(o => ({ range: this.range(o.token), newText: newName })) }] };
  }
  completion(position, snippets = false) {
    const offset = this.document.offsetAt(position), analysis = this.analysis;
    if (analysis.inTrivia(offset)) return { isIncomplete: false, items: [] };
    const prefix = this.document.getText().slice(0, offset);
    const partial = /[A-Za-z_][A-Za-z0-9_]*$/.exec(prefix)?.[0] || '';
    const start = offset - partial.length;
    const token = analysis.tokens.find(t => t.kind === 'name' && t.start <= offset && offset <= t.end);
    const wordRange = this.range({ start, end: token?.end ?? offset });
    if (analysis.isInterface) {
      const items = [...analysis.builtins.values()].filter(s => s.name.startsWith(partial)).map(s => ({
        label: s.name, kind: 7, detail: signature(s), textEdit: { range: wordRange, newText: s.name }
      }));
      for (const word of ['let', 'type']) if (word.startsWith(partial)) items.push({ label: word, kind: 14, textEdit: { range: wordRange, newText: word } });
      return { isIncomplete: false, items };
    }
    let symbols;
    const member = /\.\s*[A-Za-z0-9_]*$/.test(prefix);
    const decorator = /@\s*[A-Za-z0-9_]*$/.test(prefix);
    let capture = analysis.closures.find(c => c.node.captureOpen.end <= offset && offset <= c.node.captureClose.start);
    if (!capture) {
      const before = analysis.tokens.filter(t => t.start < offset);
      let depth = 0;
      for (let i = before.length - 1; i > 0; i--) {
        if (before[i].text === ']') depth++;
        if (before[i].text !== '[') continue;
        if (depth) { depth--; continue; }
        if (before[i - 1].text === '$') capture = { scope: { outer: analysis.scopeAt(before[i - 1].start) }, node: {
          start: before[i - 1].start, captures: before.slice(i + 1).filter(t => t.kind === 'name').map(name => ({ name }))
        } };
        break;
      }
    }
    if (member) {
      const field = analysis.fields.filter(f => f.node.receiver.end <= start && f.node.name.start <= offset && f.node.name.end >= offset)
        .sort((a, b) => b.node.start - a.node.start)[0];
      symbols = [...(field?.receiver.fields?.values() || [])];
    } else {
      const visible = new Map();
      for (let scope = capture ? capture.scope.outer : analysis.scopeAt(offset); scope; scope = scope.parent) {
        for (const [name, symbol] of scope.symbols) if (symbol.available <= (capture?.node.start ?? offset) && !visible.has(name)) visible.set(name, symbol);
      }
      if (!capture) for (const [name, symbol] of analysis.builtins) visible.set(name, symbol);
      symbols = [...visible.values()];
      if (capture) symbols = symbols.filter(s => !capture.node.captures.some(c => c.name.text === s.name && c.name.start !== token?.start));
      if (decorator) symbols = symbols.filter(s => analysis.value(s).callable);
    }
    const items = symbols.filter(s => s.name.startsWith(partial)).map(s => ({ label: s.name,
      kind: COMPLETION_KINDS[s.kind], detail: signature(s), documentation: s.doc,
      textEdit: { range: wordRange, newText: s.name } }));
    if (!member && !decorator && !capture) {
      for (const word of KEYWORDS) if (word.startsWith(partial)) items.push({ label: word, kind: 14, textEdit: { range: wordRange, newText: word } });
      if (snippets) {
        for (const [label, detail, body] of [
          ['closure', 'Callable closure', '\\$[${1:captures}](${2:parameters}) {\n\t$0\n}'],
          ['object', 'Data closure', '@{\n\tlet ${1:field} = ${2:value};\n\t$0\n}'],
          ['ifelse', 'Conditional', 'if ${1:condition} {\n\t${2}\n} else {\n\t$0\n}'],
          ['loop', 'Loop', 'loop {\n\t$0\n}'],
          ['let', 'Variable declaration', 'let ${1:name} = ${2:value};$0']
        ]) if (label.startsWith(partial)) items.push({ label, kind: 15, detail, insertTextFormat: 2, textEdit: { range: wordRange, newText: body } });
      }
    }
    return { isIncomplete: false, items };
  }
  signatureHelp(position) {
    if (this.analysis.isInterface) return null;
    const offset = this.document.offsetAt(position);
    if (this.analysis.inTrivia(offset)) return null;
    let call = this.analysis.calls.filter(c => c.node.open.end <= offset && offset <= c.node.close.start)
      .sort((a, b) => b.node.open.start - a.node.open.start)[0];
    // While typing an unfinished call, recover its callee from the resolved identifier.
    if (!call) {
      const tokens = this.analysis.tokens.filter(t => t.start < offset);
      let depth = 0, commas = 0;
      for (let i = tokens.length - 1; i >= 0; i--) {
        const token = tokens[i];
        if ([')', ']', '}'].includes(token.text)) depth++;
        else if (['(', '[', '{'].includes(token.text)) {
          if (depth) { depth--; continue; }
          if (token.text !== '(') break;
          const previous = tokens[i - 1];
          const symbol = previous && this.analysis.lookup(this.analysis.scopeAt(previous.start), previous.text, previous.start);
          if (symbol?.info.params) call = { info: symbol.info, label: symbol.name, active: commas };
          break;
        } else if (!depth && token.text === ',') commas++;
      }
    }
    if (!call?.info.params) return null;
    const params = call.info.params.map(p => `${p.byRef ? '&' : ''}${p.name.text}${p.info ? `: ${typeLabel(p.info)}` : ''}`);
    const name = call.label || this.document.getText(this.range(call.node.callee));
    const active = call.active ?? call.node.commas.filter(c => c.start < offset).length;
    const signature = { label: `${name}(${params.join(', ')})${call.info.annotated ? ` -> ${typeLabel(call.info.result)}` : ''}`, parameters: params.map(label => ({ label })) };
    return { signatures: [signature], activeSignature: 0, ...(params.length ? { activeParameter: Math.min(active, params.length - 1) } : {}) };
  }
  documentSymbols() {
    const roots = [], stack = [];
    const symbols = this.analysis.symbols.filter(s => s.role !== 'capture').sort((a, b) => a.node.start - b.node.start || b.node.end - a.node.end || a.token.start - b.token.start);
    for (const symbol of symbols) {
      const span = symbol.role === 'parameter' ? symbol.token : symbol.node;
      while (stack.length && !(stack.at(-1).span.start <= span.start && span.end <= stack.at(-1).span.end)) stack.pop();
      const item = { name: symbol.name, detail: signature(symbol), kind: SYMBOL_KINDS[symbol.kind], range: this.range(span), selectionRange: this.range(symbol.token) };
      if (stack.length) (stack.at(-1).item.children ||= []).push(item); else roots.push(item);
      if (symbol.role !== 'parameter') stack.push({ span, item });
    }
    return roots;
  }
  foldingRanges() {
    const ranges = [], seen = new Set();
    for (const body of this.analysis.blocks) {
      if (body === this.analysis.ast) continue;
      const startLine = this.document.positionAt(body.start).line, endLine = this.document.positionAt(body.end).line;
      const key = `${startLine}:${endLine}`;
      if (endLine > startLine && !seen.has(key)) { ranges.push({ startLine, endLine, kind: 'region' }); seen.add(key); }
    }
    for (const comment of this.analysis.trivia.filter(t => t.kind === 'comment')) {
      const startLine = this.document.positionAt(comment.start).line, endLine = this.document.positionAt(comment.end).line;
      if (endLine > startLine) ranges.push({ startLine, endLine, kind: 'comment' });
    }
    return ranges;
  }
  selectionRanges(positions) {
    return positions.map(position => {
      const offset = this.document.offsetAt(position);
      const spans = [...this.analysis.nodes, ...this.analysis.trivia, { start: 0, end: this.document.getText().length }]
        .filter(n => n.start <= offset && offset <= n.end)
        .sort((a, b) => b.end - b.start - (a.end - a.start));
      let parent;
      for (const span of spans) {
        const range = this.range(span);
        if (parent && JSON.stringify(parent.range) === JSON.stringify(range)) continue;
        if (parent && (this.document.offsetAt(parent.range.start) > span.start || this.document.offsetAt(parent.range.end) < span.end)) continue;
        parent = { range, ...(parent ? { parent } : {}) };
      }
      return parent || { range: { start: position, end: position } };
    });
  }
  inlayHints(range, options = {}) {
    if (this.analysis.isInterface) return [];
    const start = this.document.offsetAt(range.start), end = this.document.offsetAt(range.end);
    const hints = [];
    if (options.types !== false) for (const symbol of this.analysis.symbols) {
      const info = this.analysis.value(symbol);
      if (symbol.role !== 'variable' || symbol.token.end < start || symbol.token.end > end || (info.type === 'unknown' && (!info.displayType || info.displayType === 'any')) || info.callable) continue;
      const label = typeLabel(info);
      hints.push({ position: this.document.positionAt(symbol.token.end), kind: 1,
        label: [{ value: ': ' }, { value: label, ...(info.constructorSymbol ? { location: this.location(info.constructorSymbol.token) } : info.typeLocation ? { location: info.typeLocation } : {}) }],
        tooltip: 'Statically inferred approximate type; Prim is dynamically typed, so this hint is never written to the source.' });
    }
    if (options.parameters !== false) for (const { node, info } of this.analysis.calls) {
      if (!info.params) continue;
      node.args.forEach((arg, i) => {
        const parameter = info.params[i];
        if (!parameter || arg.expression.kind === 'error' || arg.expression.symbol?.name === parameter.name.text) return;
        const offset = arg.reference?.start ?? arg.expression.start;
        if (offset < start || offset > end) return;
        hints.push({ position: this.document.positionAt(offset), kind: 2, paddingRight: true,
          label: `${parameter.byRef ? '&' : ''}${parameter.name.text}:` });
      });
    }
    return hints.sort((a, b) => this.document.offsetAt(a.position) - this.document.offsetAt(b.position));
  }
  codeActions(range, context = {}, warnings = true) {
    if (this.analysis.isInterface) return [];
    if (context.only && !context.only.some(kind => 'quickfix'.startsWith(kind))) return [];
    const start = this.document.offsetAt(range.start), end = this.document.offsetAt(range.end);
    const actions = [];
    const add = (title, diagnostic, edits, preferred = false) => actions.push({ title, kind: 'quickfix',
      diagnostics: [diagnostic], isPreferred: preferred, edit: { documentChanges: [{
        textDocument: { uri: this.document.uri, version: this.document.version }, edits
      }] } });
    for (const diagnostic of this.diagnostics(warnings)) {
      if (this.document.offsetAt(diagnostic.range.end) < start || this.document.offsetAt(diagnostic.range.start) > end) continue;
      if (['MissingSemicolon', 'DataClosureTail'].includes(diagnostic.code) && diagnostic.data) {
        if (diagnostic.code === 'MissingSemicolon' && this.analysis.diagnostics.some(d =>
          d.start === this.document.offsetAt(diagnostic.range.start) && ['ExpectedToken', 'UnexpectedToken'].includes(d.code))) continue;
        const offset = diagnostic.data.insert;
        add('Add missing semicolon', diagnostic, [{ range: this.range({ start: offset, end: offset }), newText: ';' }], true);
      } else if (diagnostic.code === 'MissingCapture') {
        const closure = this.analysis.closures.find(c => c.node.start === diagnostic.data.capture)?.node;
        if (!closure) continue;
        const preceding = this.analysis.tokens.filter(t => t.end <= closure.captureClose.start).at(-1);
        const separator = preceding.text === '[' ? '' : preceding.text === ',' ? ' ' : ', ';
        for (const mode of ['', '&']) add(`Add ${mode}${diagnostic.data.name} to the capture list`, diagnostic, [{
          range: this.range({ start: closure.captureClose.start, end: closure.captureClose.start }), newText: separator + mode + diagnostic.data.name
        }]);
      } else if (diagnostic.code === 'UnusedBinding') {
        const symbol = this.at(diagnostic.range.start)?.symbol;
        if (!symbol) continue;
        try {
          const edit = this.rename(diagnostic.range.start, `_${symbol.name}`);
          actions.push({ title: `Rename to _${symbol.name} to mark it as intentionally kept`, kind: 'quickfix', diagnostics: [diagnostic], edit });
        } catch { /* A quick fix must not introduce a binding conflict. */ }
      }
    }
    return actions;
  }
  semanticTokens() {
    const data = [], used = new Set();
    let previousLine = 0, previousCharacter = 0;
    for (const occurrence of [...this.analysis.occurrences].sort((a, b) => a.token.start - b.token.start)) {
      const { token, symbol } = occurrence;
      if (used.has(token.start) || token.end <= token.start) continue;
      const kind = symbol?.kind || (occurrence.property ? 'property' : 'variable');
      const position = this.document.positionAt(token.start);
      const deltaLine = position.line - previousLine;
      const modifiers = (occurrence.declaration ? 1 : 0) | (symbol?.readonly ? 2 : 0) | (symbol?.builtin ? 4 : 0);
      data.push(deltaLine, deltaLine ? position.character : position.character - previousCharacter,
        token.end - token.start, TOKEN_TYPES.indexOf(kind), modifiers);
      previousLine = position.line; previousCharacter = position.character; used.add(token.start);
    }
    return { data };
  }
}

module.exports = { Features, TOKEN_TYPES, TOKEN_MODIFIERS };
