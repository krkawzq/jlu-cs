'use strict';

const { TextDocument } = require('vscode-languageserver-textdocument');
const { lex, Parser } = require('./syntax');
const PRIMITIVES = new Set(['int', 'float', 'bool', 'str', 'unit', 'closure', 'callable', 'any']);

class InterfaceParser extends Parser {
  declarations(braced = false) {
    const declarations = [];
    while (this.token.kind !== 'eof' && !(braced && this.is('}'))) {
      const before = this.pos;
      try {
        const keyword = this.take(), start = keyword.start;
        if (!['let', 'type'].includes(keyword.text) || (braced && keyword.text !== 'let')) {
          this.report('InterfaceSyntax', braced ? 'Object types require let field declarations.' : '.pri files accept only let or type declarations; expressions are not executed.', keyword);
          throw new Error('declaration');
        }
        const name = this.name();
        let value = null;
        if (keyword.text === 'let') { this.expect(':'); value = this.typeExpression(); }
        else if (this.eat('=')) value = this.typeExpression();
        this.expect(';');
        declarations.push(this.node(keyword.text, start, { name, value }));
      } catch {
        let depth = 0;
        while (this.token.kind !== 'eof') {
          if (!depth && (this.is(';') || (braced && this.is('}')) || (this.pos > before && ['let', 'type'].includes(this.token.text)))) break;
          if (this.is('{')) depth++;
          if (this.is('}')) depth = Math.max(0, depth - 1);
          this.take();
        }
        this.eat(';');
      }
      if (this.pos === before) this.take();
    }
    return declarations;
  }
  typeExpression() {
    if (++this.depth > 128) {
      this.depth--;
      this.report('TypeNestingLimit', 'Type expression is too deeply nested.', this.token);
      throw new Error('nesting');
    }
    try {
      const start = this.token.start, options = [this.typePrimary()];
      while (this.eat('|')) options.push(this.typePrimary());
      return options.length === 1 ? options[0] : this.node('unionType', start, { options });
    } finally { this.depth--; }
  }
  typePrimary() {
    const start = this.token.start;
    if (this.eat('$')) {
      this.expect('['); this.expect(']'); this.expect('(');
      const params = [];
      if (!this.is(')')) do {
        const byRef = !!this.eat('&'), name = this.name(); this.expect(':');
        const value = this.typeExpression(); params.push({ name, byRef, value });
        if (this.is(')')) break;
        this.expect(',');
      } while (!this.is(')') && this.token.kind !== 'eof');
      this.expect(')'); this.expect('->');
      const result = this.typeExpression();
      return this.node('functionType', start, { params, result });
    }
    if (this.eat('@')) {
      this.expect('{'); const fields = this.declarations(true); this.expect('}');
      return this.node('recordType', start, { fields });
    }
    if (this.eat('(')) {
      const value = this.typeExpression(); this.expect(')');
      return this.node('groupType', start, { value });
    }
    const name = this.name();
    return this.node('namedType', start, { name });
  }
}

function primitive(name) {
  return { type: name === 'any' ? 'unknown' : name === 'callable' ? 'closure' : name,
    displayType: name, ...(name === 'callable' ? { callable: true } : {}) };
}

function typeLabel(info) {
  return info?.displayType || info?.constructorSymbol?.name || info?.type || 'unknown';
}

class InterfaceAnalysis {
  constructor(source) {
    this.isInterface = true; this.text = source.text; this.uri = source.uri;
    this.document = TextDocument.create(source.uri, 'prim-interface', source.version || 0, source.text);
    this.diagnostics = []; this.symbols = []; this.occurrences = []; this.trivia = [];
    this.nodes = []; this.blocks = []; this.calls = []; this.fields = []; this.closures = [];
    this.report = (code, message, span, severity = 1) => {
      if (this.diagnostics.length < 100) this.diagnostics.push({ code, message, start: span.start, end: span.end, severity });
    };
    this.tokens = lex(source.text, this.report, this.trivia, { interface: true });
    this.declarations = new InterfaceParser(this.tokens, this.report).declarations();
    this.ast = { start: 0, end: source.text.length };
    const pending = [...this.declarations];
    while (pending.length) {
      const node = pending.pop();
      if (!node || typeof node !== 'object') continue;
      if (typeof node.start === 'number') this.nodes.push(node);
      if (node.kind === 'recordType') this.blocks.push(node);
      for (const value of Object.values(node)) {
        if (Array.isArray(value)) for (const child of value) pending.push(child);
        else if (value && typeof value === 'object') pending.push(value);
      }
    }
    this.top = { start: 0, end: source.text.length, symbols: new Map() };
    this.scopes = [this.top]; this.builtins = new Map();
  }
  location(span) { return { uri: this.uri, range: { start: this.document.positionAt(span.start), end: this.document.positionAt(span.end) } }; }
  doc(node) {
    let end = node.start;
    const comments = [];
    for (const item of [...this.trivia].reverse()) {
      if (item.end > end) continue;
      if (this.text.slice(item.end, end).trim() || /(?:\r?\n)[ \t]*(?:\r?\n)/.test(this.text.slice(item.end, end))) break;
      if (item.kind !== 'comment') break;
      comments.unshift(this.text.slice(item.start, item.end).replace(/^\/\/\/?\s?/, '').replace(/^\/\*|\*\/$/g, '').trim());
      end = item.start;
    }
    return comments.join('\n');
  }
  symbol(node, scope, kind, info, role = 'variable') {
    const symbol = { name: node.name.text, token: node.name, scope, node, info, kind, role, available: 0,
      location: this.location(node.name), doc: this.doc(node), external: true, interfaceDeclaration: true };
    this.symbols.push(symbol); this.occurrences.push({ token: node.name, symbol, scope, declaration: true });
    return symbol;
  }
  value(symbol) { return symbol?.info || primitive('any'); }
  occurrenceAt(offset) { return this.occurrences.find(o => o.token.start <= offset && offset < o.token.end) || this.occurrences.find(o => o.token.end === offset); }
  references(symbol) { return this.occurrences.filter(o => o.symbol === symbol); }
  scopeAt() { return this.top; }
  lookup(scope, name) { return scope.symbols.get(name) || this.builtins.get(name); }
  inTrivia(offset) { return this.trivia.some(t => t.start < offset && (offset < t.end || (offset === t.end && (t.line || !t.closed)))); }
}

function createInterfaces(sources) {
  const analyses = sources.map(s => new InterfaceAnalysis(s));
  const aliases = new Map(), values = new Map();
  for (const analysis of analyses) for (const node of analysis.declarations) {
    if (node.kind !== 'type') continue;
    if (aliases.has(node.name.text)) analysis.report('DuplicateType', `Duplicate type declaration ${node.name.text}.`, node.name);
    const symbol = analysis.symbol(node, analysis.top, 'type', primitive('any'), 'type');
    aliases.set(node.name.text, { node, analysis, symbol });
    analysis.top.symbols.set(symbol.name, symbol);
  }
  const resolving = new Set(), resolved = new Set();
  function alias(name) {
    const entry = aliases.get(name);
    if (!entry) return PRIMITIVES.has(name) ? primitive(name) : null;
    if (resolved.has(name)) return entry.symbol.info;
    if (resolving.has(name)) {
      entry.analysis.report('RecursiveType', `Recursive type alias ${name} is not supported.`, entry.node.name);
      return primitive('any');
    }
    resolving.add(name);
    const info = entry.node.value ? resolve(entry.node.value, entry.analysis) : PRIMITIVES.has(name) ? primitive(name) : primitive('any');
    entry.symbol.info = { ...info, displayType: name, typeLocation: entry.symbol.location };
    resolving.delete(name); resolved.add(name);
    return entry.symbol.info;
  }
  function resolve(node, analysis) {
    if (node.kind === 'namedType') {
      const info = alias(node.name.text);
      const symbol = aliases.get(node.name.text)?.symbol || { name: node.name.text, kind: 'type', builtin: true, info: info || primitive('any') };
      analysis.occurrences.push({ token: node.name, symbol, scope: analysis.top });
      if (!info) analysis.report('UndefinedType', `Unknown type ${node.name.text}.`, node.name);
      return info || primitive('any');
    }
    if (node.kind === 'groupType') return resolve(node.value, analysis);
    if (node.kind === 'unionType') {
      const options = node.options.map(n => resolve(n, analysis));
      return { type: 'unknown', displayType: options.map(typeLabel).join(' | '), options };
    }
    if (node.kind === 'functionType') {
      const names = new Set();
      const params = node.params.map(p => {
        if (names.has(p.name.text)) analysis.report('DuplicateBinding', `Duplicate parameter ${p.name.text}.`, p.name);
        names.add(p.name.text);
        const info = resolve(p.value, analysis);
        analysis.symbol({ ...node, name: p.name }, analysis.top, 'parameter', info, 'parameter');
        return { name: p.name, byRef: p.byRef, info };
      });
      return { type: 'closure', callable: true, params, result: resolve(node.result, analysis), annotated: true };
    }
    if (node.kind === 'recordType') {
      const fields = new Map();
      for (const field of node.fields) {
        const info = resolve(field.value, analysis);
        if (fields.has(field.name.text)) analysis.report('DuplicateBinding', `Duplicate field ${field.name.text}.`, field.name);
        fields.set(field.name.text, analysis.symbol(field, analysis.top, info.callable ? 'function' : 'property', info, 'property'));
      }
      return { type: 'closure', fields, typeLocation: analysis.location(node) };
    }
    return primitive('any');
  }
  for (const name of aliases.keys()) alias(name);
  for (const analysis of analyses) {
    for (const node of analysis.declarations.filter(n => n.kind === 'let')) {
      const info = resolve(node.value, analysis);
      if (values.has(node.name.text)) analysis.report('DuplicateBinding', `Duplicate interface declaration ${node.name.text}.`, node.name);
      const symbol = analysis.symbol(node, analysis.top, info.callable ? 'function' : info.fields ? 'namespace' : 'variable', info);
      values.set(symbol.name, symbol); analysis.top.symbols.set(symbol.name, symbol);
    }
    analysis.builtins = new Map([...PRIMITIVES].map(name => [name, aliases.get(name)?.symbol || { name, kind: 'type', builtin: true, info: primitive(name) }]));
    for (const [name, entry] of aliases) analysis.builtins.set(name, entry.symbol);
  }
  return { analyses: new Map(analyses.map(a => [a.uri, a])), values, types: new Map([...aliases].map(([name, e]) => [name, e.symbol])) };
}

module.exports = { createInterfaces, InterfaceAnalysis, PRIMITIVES, typeLabel };
