'use strict';

const { lex, Parser } = require('./syntax');
const { createBuiltins } = require('./builtins');
const { typeLabel } = require('./interfaces');
const UNKNOWN = Object.freeze({ type: 'unknown' });
const UNIT = Object.freeze({ type: 'unit' });
const primitive = type => ({ type });
const unwrap = e => e?.kind === 'group' ? unwrap(e.value) : e;
const rootSymbol = symbol => symbol?.source ? rootSymbol(symbol.source) : symbol;

class Analysis {
  constructor(text, invalidated = null, options = {}) {
    this.options = options;
    this.hints = options.hints || new Map();
    this.text = text; this.diagnostics = []; this.symbols = []; this.occurrences = [];
    this.scopes = []; this.calls = []; this.fields = []; this.blocks = []; this.closures = [];
    this.trivia = []; this.nodes = []; this.mutated = new Set(); this.invalidated = invalidated || new Set();
    this.expressionDepth = 0;
    this.report = (code, message, span, severity = 1, tags, data) => {
      if (this.diagnostics.length < 100) this.diagnostics.push({ code, message, start: span.start, end: span.end, severity, tags, data });
    };
    this.tokens = lex(text, this.report, this.trivia);
    this.ast = new Parser(this.tokens, this.report).body();
    const pending = [this.ast];
    while (pending.length) {
      const node = pending.pop();
      if (!node || typeof node !== 'object') continue;
      if (typeof node.start === 'number' && typeof node.end === 'number') this.nodes.push(node);
      for (const value of Object.values(node)) {
        if (Array.isArray(value)) for (const child of value) pending.push(child);
        else if (value && typeof value === 'object') pending.push(value);
      }
    }
    this.builtins = options.builtins || createBuiltins();
    this.top = this.scope(null, this.ast);
    this.body(this.ast, this.top, { loops: 0, returns: null, self: null });
    // Resolve writes to declaration identities before inferring values, including
    // writes in deferred closures and through fields of constructed objects.
    if (invalidated === null && this.mutated.size) return new Analysis(text, this.mutated, { ...options, builtins: this.builtins });
    const reads = new Set(this.occurrences.filter(o => o.symbol && ((!o.declaration && !o.write) || o.symbol.source)).map(o => rootSymbol(o.symbol)));
    for (const symbol of this.symbols) {
      if (['property', 'capture'].includes(symbol.role) || symbol.name.startsWith('_')) continue;
      const read = reads.has(rootSymbol(symbol));
      if (!read) this.report('UnusedBinding', `${symbol.name} is never read; prefix it with _ if it is intentionally kept.`, symbol.token, 2, [1]);
    }
    this.referenceIndex = new Map();
    for (const occurrence of this.occurrences) {
      if (!occurrence.symbol) continue;
      const root = rootSymbol(occurrence.symbol);
      if (!this.referenceIndex.has(root)) this.referenceIndex.set(root, []);
      this.referenceIndex.get(root).push(occurrence);
    }
  }
  invalidate(symbol) {
    if (symbol?.token && !symbol.external && !symbol.builtin) this.mutated.add(symbol.token.start);
  }
  scope(parent, node, outer = null) {
    const scope = { parent, outer, start: node.start, end: node.end, symbols: new Map() };
    this.scopes.push(scope); return scope;
  }
  lookup(scope, name, offset = Infinity) {
    for (let s = scope; s; s = s.parent) {
      const symbol = s.symbols.get(name);
      if (symbol && symbol.available <= offset) return symbol;
    }
    return this.builtins.get(name);
  }
  use(token, scope, write = false) {
    const symbol = this.lookup(scope, token.text);
    if (!symbol) {
      let boundary = scope;
      while (boundary.parent) boundary = boundary.parent;
      const missingCapture = boundary.outer && this.lookup(boundary.outer, token.text, boundary.closure.start);
      this.report(missingCapture ? 'MissingCapture' : 'UndefinedName', missingCapture
        ? `${token.text} comes from an outer scope and must be added to the closure capture list.`
        : `Undefined name ${token.text}.`, token, 1, undefined,
        missingCapture ? { capture: boundary.closure.start, name: token.text } : undefined);
    }
    this.occurrences.push({ token, symbol, scope, write });
    return symbol;
  }
  define(token, scope, info, role, node, source = null) {
    if (this.builtins.has(token.text)) this.report('ReservedBinding', `${token.text} is a reserved name and cannot be shadowed.`, token);
    if (scope.symbols.has(token.text)) this.report('DuplicateBinding', `${token.text} is already declared in this scope.`, token);
    else if (scope.parent && this.lookup(scope.parent, token.text)) {
      this.report('ShadowedBinding', `${token.text} shadows a binding with the same name in an outer scope.`, token, 2);
    }
    if (this.invalidated.has(token.start)) info = UNKNOWN;
    const symbol = { name: token.text, token, scope, info, role, node, source,
      available: role === 'parameter' || role === 'capture' ? scope.start : node.end };
    symbol.kind = role === 'parameter' ? 'parameter' : role === 'property' ? 'property'
      : info.callable ? (/^[A-Z]/.test(symbol.name) && info.result?.fields && !info.result.callable ? 'type' : 'function') : 'variable';
    scope.symbols.set(symbol.name, symbol); this.symbols.push(symbol);
    this.occurrences.push({ token, symbol, scope, declaration: true });
    return symbol;
  }
  value(symbol) {
    if (!symbol || (!symbol.builtin && !symbol.external && this.invalidated.has(symbol.token?.start))) return UNKNOWN;
    return symbol.info;
  }
  body(body, scope, context) {
    this.blocks.push(body);
    let terminated = false;
    for (const stmt of body.statements) {
      if (terminated) this.report('UnreachableCode', 'Unreachable statement after a control transfer.', stmt, 2, [1]);
      if (stmt.kind === 'let') {
        const hint = scope === this.top ? this.hints.get(stmt.name.text) : context.dataScope === scope ? context.dataHints?.get(stmt.name.text) : null;
        if (hint) this.annotate(stmt.value.expression, hint.info);
        const value = this.expression(stmt.value.expression, scope, context);
        if (stmt.value.byRef) { this.place(stmt.value.expression); this.invalidate(unwrap(stmt.value.expression).symbol); }
        const declared = hint && !value.annotated && !value.interfaceMismatch ? hint.info : value;
        const symbol = this.define(stmt.name, scope, declared, context.dataScope === scope ? 'property' : 'variable', stmt);
        if (hint) symbol.hintLocation = hint.location;
      } else if (stmt.kind === 'assign') {
        this.expression(stmt.target, scope, context, true); this.place(stmt.target);
        this.invalidate(unwrap(stmt.target).symbol);
        this.expression(stmt.value, scope, context);
      } else if (['return', 'break', 'continue'].includes(stmt.kind)) {
        const valid = stmt.kind === 'return' ? context.returns : context.loops;
        if (!valid) this.report('InvalidControlTarget', `${stmt.kind} has no valid target in the current execution boundary.`, stmt.keyword);
        const info = stmt.value ? this.expression(stmt.value, scope, context) : UNIT;
        if (stmt.kind === 'return' && context.returns) context.returns.push(info);
        terminated = true;
      } else this.expression(stmt.value, scope, context);
    }
    if (body.tail) {
      if (terminated) this.report('UnreachableCode', 'Unreachable expression after a control transfer.', body.tail, 2, [1]);
      const result = this.expression(body.tail, scope, context);
      return terminated ? UNKNOWN : result;
    }
    return terminated ? UNKNOWN : UNIT;
  }
  place(node) {
    const e = unwrap(node);
    if (!['name', 'field'].includes(e.kind)) this.report('InvalidPlace', 'Expected a writable variable or field.', node);
    else if (e.symbol?.readonly) {
      if (e.kind === 'name') this.report('InvalidPlace', 'Reserved names are read-only.', node);
      else this.report('ReadOnlySlot', 'This standard library field is read-only; prim-rs checks field writability at runtime.', node, 2);
    }
  }
  annotate(node, info) {
    if (node.kind === 'group' || node.kind === 'decorate') this.annotate(node.value || node.target, info);
    else node.annotation = info;
  }
  expression(e, scope, context, write = false) {
    if (++this.expressionDepth > 256) {
      this.expressionDepth--;
      this.report('AnalysisLimit', 'Expression is too deeply nested for the editor to infer this subexpression; prim-rs may still accept the program.', e, 2);
      return UNKNOWN;
    }
    try { return this.expressionInner(e, scope, context, write); } finally { this.expressionDepth--; }
  }
  expressionInner(e, scope, context, write = false) {
    if (!e) return UNIT;
    let info = UNKNOWN;
    switch (e.kind) {
      case 'int': {
        const raw = e.token.text.replaceAll('_', '');
        const n = BigInt(raw);
        if (n > 9223372036854775807n && !e.allowMin) this.report('IntegerLiteralOutOfRange', 'Integer literal is outside the i64 range.', e);
        info = primitive('int'); break;
      }
      case 'float':
        if (!Number.isFinite(Number(e.token.text.replaceAll('_', '')))) this.report('FloatLiteralOutOfRange', 'Float literal must be finite.', e);
        info = primitive('float'); break;
      case 'str': case 'bool': case 'unit': info = primitive(e.kind); break;
      case 'name': e.symbol = this.use(e.token, scope, write); info = this.value(e.symbol); break;
      case 'self':
        if (!context.self) this.report('InvalidSelf', 'self can only be used inside a callable closure.', e);
        info = context.self || UNKNOWN; break;
      case 'group': info = this.expression(e.value, scope, context, write); break;
      case 'unary': {
        const value = unwrap(e.value);
        if (e.op.text === '-' && value.kind === 'int' && BigInt(value.token.text.replaceAll('_', '')) === 9223372036854775808n) value.allowMin = true;
        const operand = this.expression(e.value, scope, context);
        info = e.op.text === '!' ? primitive('bool') : operand; break;
      }
      case 'binary': {
        const left = this.expression(e.left, scope, context), right = this.expression(e.right, scope, context);
        info = ['==', '!=', '<', '<=', '>', '>=', '&&', '||'].includes(e.op.text) ? primitive('bool') : left.type === right.type ? left : UNKNOWN;
        break;
      }
      case 'field': {
        const receiver = this.expression(e.receiver, scope, context);
        e.symbol = receiver.fields?.get(e.name.text);
        if (!e.symbol && e.name.text && receiver.fields) this.report('UnknownField', `This object has no known field ${e.name.text}.`, e.name, 2);
        this.fields.push({ node: e, receiver });
        if (e.name.text) this.occurrences.push({ token: e.name, symbol: e.symbol, scope, write, property: true });
        info = this.value(e.symbol); break;
      }
      case 'call': {
        const callee = this.expression(e.callee, scope, context);
        this.calls.push({ node: e, info: callee });
        const args = e.args.map(arg => {
          const value = this.expression(arg.expression, scope, context);
          if (arg.byRef) { this.place(arg.expression); this.invalidate(unwrap(arg.expression).symbol); }
          return value;
        });
        info = unwrap(e.callee).symbol === this.builtins.get('jit') ? args[0] || UNKNOWN : callee.result || UNKNOWN;
        const constructor = unwrap(e.callee).symbol;
        if (info.fields && !info.callable && constructor?.kind === 'type') info = { ...info, constructorSymbol: constructor };
        if (!e.incomplete && callee.params) {
          if (callee.params.length !== e.args.length) this.report('CallArity', `This call expects ${callee.params.length} arguments but received ${e.args.length}.`, e, 2);
          for (let i = 0; i < Math.min(callee.params.length, e.args.length); i++) {
            if (callee.params[i].byRef !== e.args[i].byRef) this.report('ArgumentMode', callee.params[i].byRef
              ? `Parameter ${callee.params[i].name.text} requires a & reference argument.` : `Parameter ${callee.params[i].name.text} takes its argument by value and cannot receive a & reference.`, e.args[i].expression, 2);
          }
        }
        break;
      }
      case 'block': info = this.body(e.body, this.scope(scope, e.body), context); break;
      case 'if': {
        this.expression(e.condition, scope, context);
        const yes = this.body(e.body, this.scope(scope, e.body), context);
        const no = e.other ? this.expression(e.other, scope, context) : UNIT;
        info = yes === no ? yes : yes.type === no.type && !yes.fields && !yes.callable ? primitive(yes.type) : UNKNOWN;
        break;
      }
      case 'loop': this.body(e.body, this.scope(scope, e.body), { ...context, loops: context.loops + 1 }); break;
      case 'data': {
        const inner = this.scope(scope, e.body);
        this.body(e.body, inner, { ...context, loops: 0, returns: null, dataScope: inner, dataHints: e.annotation?.fields });
        info = { type: 'closure', fields: new Map(inner.symbols) }; break;
      }
      case 'closure': {
        const captures = e.captures.map(c => ({ ...c, source: this.use(c.name, scope) }));
        const inner = this.scope(null, e.body, scope);
        inner.closure = e; this.closures.push({ node: e, scope: inner });
        const hint = e.annotation;
        const compatible = hint?.callable && (!hint.params || (hint.params.length === e.params.length && hint.params.every((p, i) => p.byRef === e.params[i].byRef)));
        if (hint && !compatible) this.report('InterfaceMismatch', ".pri declaration does not match the closure's parameter count or reference modes.", e, 2);
        const params = e.params.map((p, i) => ({ ...p, ...(compatible && hint.params?.[i]?.info ? { info: hint.params[i].info } : {}) }));
        info = { type: 'closure', callable: true, params, fields: new Map(), result: UNKNOWN,
          annotated: !!compatible, interfaceMismatch: !!hint && !compatible };
        for (const c of captures) {
          if (c.byRef) this.invalidate(c.source);
          const captured = this.define(c.name, inner, c.byRef ? UNKNOWN : this.value(c.source), 'capture', e, c.source);
          // A capture token both reads the outer binding and declares its local field.
          this.occurrences = this.occurrences.filter(o => !(o.token === c.name && o.symbol === c.source && !o.declaration));
          info.fields.set(c.name.text, captured);
        }
        for (const p of params) this.define(p.name, inner, p.info || UNKNOWN, 'parameter', e);
        const returns = [];
        const tail = this.body(e.body, inner, { loops: 0, returns, self: info });
        info.result = compatible && hint.result ? hint.result : returns.length ? (returns.every(v => v === returns[0]) && (tail === UNKNOWN || tail === returns[0]) ? returns[0] : UNKNOWN) : tail;
        break;
      }
      case 'decorate': {
        const decorator = this.use(e.name, scope);
        const target = this.expression(e.target, scope, context);
        // User decorators may replace both the signature and returned object shape.
        info = decorator === this.builtins.get('jit') ? target : { type: 'closure', callable: true };
        break;
      }
    }
    e.info = info; return info;
  }
  occurrenceAt(offset) {
    return this.occurrences.find(o => o.token.start <= offset && offset < o.token.end)
      || this.occurrences.find(o => o.token.end === offset);
  }
  scopeAt(offset) {
    return this.scopes.filter(s => s.start <= offset && offset <= s.end)
      .sort((a, b) => (a.end - a.start) - (b.end - b.start))[0] || this.top;
  }
  references(symbol) {
    return this.referenceIndex.get(rootSymbol(symbol)) || [];
  }
  inTrivia(offset) {
    return this.trivia.some(t => t.start < offset && (offset < t.end || (offset === t.end && (t.line || !t.closed))));
  }
}

function signature(symbol) {
  if (!symbol) return '';
  const info = symbol.info || UNKNOWN;
  if (info.callable) {
    const args = info.params?.map(p => `${p.byRef ? '&' : ''}${p.name.text}${p.info ? `: ${typeLabel(p.info)}` : ''}`).join(', ');
    return `${symbol.name}(${args ?? '…'}) -> ${typeLabel(info.result)}`;
  }
  return `${symbol.name}: ${typeLabel(info)}`;
}

module.exports = { Analysis, UNKNOWN, rootSymbol, signature };
