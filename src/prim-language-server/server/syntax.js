'use strict';

// Offsets are UTF-16 code units, matching LSP's default position encoding.
const KEYWORDS = new Set('let if else loop break continue return true false self'.split(' '));
const PRECEDENCE = { '||': 10, '&&': 20, '==': 30, '!=': 30, '<': 30, '<=': 30,
  '>': 30, '>=': 30, '+': 40, '-': 40, '*': 50, '/': 50, '%': 50 };

function lex(source, report, trivia = [], options = {}) {
  const tokens = [];
  let i = 0;
  for (let p = 0; p < source.length; p++) {
    if (source[p] === '\0' || (p === 0 && source[p] === '\uFEFF')) {
      report('ForbiddenSourceCharacter', 'Source must not contain a BOM or a raw NUL.', { start: p, end: p + 1 });
    }
  }
  while (i < source.length) {
    const start = i;
    const c = source[i];
    if (/[ \t\r\n]/.test(c)) { i++; continue; }
    if (source.startsWith('//', i)) {
      while (i < source.length && !/[\r\n]/.test(source[i])) i++;
      trivia.push({ kind: 'comment', start, end: i, line: true });
      continue;
    }
    if (source.startsWith('/*', i)) {
      const end = source.indexOf('*/', i + 2);
      i = end < 0 ? source.length : end + 2;
      trivia.push({ kind: 'comment', start, end: i, closed: end >= 0 });
      if (end < 0) report('UnterminatedComment', 'Block comment is missing its closing */.', { start, end: i });
      continue;
    }
    let kind;
    if (/[A-Za-z_]/.test(c)) {
      i++;
      while (i < source.length && /[A-Za-z0-9_]/.test(source[i])) i++;
      kind = KEYWORDS.has(source.slice(start, i)) ? 'keyword' : 'name';
    } else if (/[0-9]/.test(c)) {
      const prefixed = /^0[xXoObB]/.test(source.slice(i, i + 2));
      i += prefixed ? 2 : 1;
      let dot = false, exponent = false;
      while (i < source.length) {
        if (!prefixed && !exponent && /[eE]/.test(source[i])) {
          exponent = true; i++;
          if (/[+-]/.test(source[i] || ' ')) i++;
        } else if (/[A-Za-z0-9_]/.test(source[i])) i++;
        else if (!prefixed && !dot && !exponent && source[i] === '.' && /[0-9]/.test(source[i + 1] || ' ')) {
          dot = true; i++;
        } else break;
      }
      const raw = source.slice(start, i);
      const valid = /^(?:0[xX][\da-fA-F](?:_?[\da-fA-F])*|0[oO][0-7](?:_?[0-7])*|0[bB][01](?:_?[01])*|\d(?:_?\d)*(?:\.\d(?:_?\d)*)?(?:[eE][+-]?\d(?:_?\d)*)?)$/.test(raw);
      kind = !prefixed && (dot || exponent) ? 'float' : 'int';
      if (!valid) { report('InvalidNumber', `Invalid number ${raw}.`, { start, end: i }); kind = 'invalid'; }
    } else if (c === '"') {
      i++;
      let closed = false;
      while (i < source.length && !/[\r\n]/.test(source[i])) {
        if (source[i] === '"') { i++; closed = true; break; }
        if (source[i] === '\\') {
          const escapeStart = i++;
          if ('"\\nrt0'.includes(source[i] || '\uFFFF')) { i++; continue; }
          const unicode = /^u\{([\da-fA-F]{1,6})\}/.exec(source.slice(i));
          if (unicode) {
            i += unicode[0].length;
            const value = parseInt(unicode[1], 16);
            if (value <= 0x10ffff && !(value >= 0xd800 && value <= 0xdfff)) continue;
          } else if (i < source.length && !/[\r\n"]/.test(source[i])) i++;
          report('InvalidEscape', 'Invalid string escape or Unicode scalar value.', { start: escapeStart, end: i });
        } else {
          if (source.charCodeAt(i) < 32 || source.charCodeAt(i) === 127) {
            report('InvalidEscape', 'Control characters in strings must be escaped.', { start: i, end: i + 1 });
          }
          i++;
        }
      }
      if (!closed) report('UnterminatedString', 'String is missing its closing double quote.', { start, end: i });
      trivia.push({ kind: 'string', start, end: i, closed });
      kind = 'str';
    } else {
      const pair = source.slice(i, i + 2);
      if (['==', '!=', '<=', '>=', '&&', '||'].includes(pair) || (options.interface && pair === '->')) i += 2;
      else i++;
      kind = 'symbol';
      if (!'=+-*/%<>!&()[]{}.,;@$'.includes(c) && !['&&', '||'].includes(pair) && !(options.interface && ':|'.includes(c))) {
        if (source.codePointAt(start) > 0xffff) i = start + 2;
        report('IllegalCharacter', `Unsupported character ${JSON.stringify(source.slice(start, i))}.`, { start, end: i });
        continue;
      }
    }
    tokens.push({ kind, text: source.slice(start, i), start, end: i });
  }
  tokens.push({ kind: 'eof', text: '', start: i, end: i });
  return tokens;
}

class ParseError extends Error {}

class Parser {
  constructor(tokens, report) { this.tokens = tokens; this.report = report; this.pos = 0; this.depth = 0; }
  get token() { return this.tokens[this.pos]; }
  is(text) { return this.token.text === text; }
  take() { const t = this.token; if (t.kind !== 'eof') this.pos++; return t; }
  eat(text) { if (this.is(text)) return this.take(); return null; }
  expect(text) {
    if (this.is(text)) return this.take();
    const insertion = this.tokens[Math.max(0, this.pos - 1)].end;
    this.report(text === ';' ? 'MissingSemicolon' : 'ExpectedToken', `Expected ${text}, found ${this.token.text || 'end of file'}.`, this.token, 1, undefined,
      text === ';' ? { insert: insertion } : undefined);
    if (text === ';' && (['let', 'return', 'break', 'continue', '}'].includes(this.token.text) || this.token.kind === 'eof')) return this.token;
    throw new ParseError();
  }
  name() {
    if (this.token.kind === 'name') return this.take();
    this.report('ExpectedIdentifier', 'Expected an identifier.', this.token);
    throw new ParseError();
  }
  node(kind, start, fields = {}) {
    const childEnds = Object.values(fields).map(value => value?.end ?? value?.expression?.end ?? start);
    return { kind, start, end: Math.max(start, this.tokens[Math.max(0, this.pos - 1)].end, ...childEnds), ...fields };
  }
  body(braced = false, data = false) {
    const start = this.token.start;
    if (braced) this.expect('{');
    const statements = [];
    let tail = null;
    while (this.token.kind !== 'eof' && !(braced && this.is('}'))) {
      const before = this.pos;
      try {
        const s = this.token.start;
        if (this.eat('let')) {
          const name = this.name(); this.expect('=');
          const value = this.argument(); this.expect(';');
          statements.push(this.node('let', s, { name, value }));
        } else if (['return', 'break', 'continue'].includes(this.token.text)) {
          const keyword = this.take();
          const value = keyword.text === 'continue' || this.is(';') ? null : this.expr();
          this.expect(';');
          statements.push(this.node(keyword.text, s, { value, keyword }));
        } else {
          const value = this.expr();
          if (this.eat('=')) {
            const right = this.expr(); this.expect(';');
            statements.push(this.node('assign', s, { target: value, value: right }));
          } else if (this.eat(';')) statements.push(this.node('expression', s, { value }));
          else if (this.token.kind === 'eof' || (braced && this.is('}'))) {
            if (data) this.report('DataClosureTail', 'A data closure cannot contain a trailing expression; add a semicolon.', value, 1, undefined, { insert: value.end });
            tail = value; break;
          } else {
            this.expect(';');
            statements.push(this.node('expression', s, { value }));
          }
        }
      } catch (error) {
        if (!(error instanceof ParseError)) throw error;
        // Recover at statement boundaries without swallowing balanced nested bodies.
        let nesting = 0;
        while (this.token.kind !== 'eof') {
          if (!nesting && (this.is('}') || this.is(';') || (this.is('let') && this.pos > before))) break;
          if (['{', '(', '['].includes(this.token.text)) nesting++;
          if (['}', ')', ']'].includes(this.token.text)) nesting = Math.max(0, nesting - 1);
          this.take();
        }
        this.eat(';');
      }
      if (this.pos === before) this.take();
    }
    let end = this.token.end;
    if (braced) {
      const close = this.eat('}');
      if (!close) this.report('ExpectedToken', 'Missing closing } for opening {.', this.token);
      else end = close.end;
    }
    return this.node('body', start, { statements, tail, end });
  }
  argument() { const reference = this.eat('&'); return { byRef: !!reference, reference, expression: this.expr(reference ? 70 : 0) }; }
  bindings(end) {
    const values = [];
    if (this.eat(end)) return values;
    do {
      const byRef = !!this.eat('&'); values.push({ name: this.name(), byRef });
      if (this.is(end)) break;
      this.expect(',');
    } while (!this.is(end) && this.token.kind !== 'eof');
    this.expect(end); return values;
  }
  callable() {
    const decorators = [];
    while (this.is('@')) { const start = this.take().start; decorators.push({ start, name: this.name() }); }
    const start = this.token.start;
    this.expect('$');
    if (this.token.kind === 'name') {
      this.report('SyntaxError', `prim-rs does not support the named function form $${this.token.text}(...); use let ${this.token.text} = $[](...) { ... }; without type annotations on parameters or the return value.`, this.token);
      throw new ParseError();
    }
    const captureOpen = this.expect('[');
    const captures = this.bindings(']'); const captureClose = this.tokens[this.pos - 1]; this.expect('(');
    const params = this.bindings(')'); const body = this.body(true);
    let target = this.node('closure', start, { captures, params, body, captureOpen, captureClose });
    for (const decorator of decorators.reverse()) target = this.node('decorate', decorator.start, { name: decorator.name, target });
    return target;
  }
  ifExpression() {
    const start = this.expect('if').start;
    const condition = this.expr(); const body = this.body(true);
    const other = this.eat('else') ? (this.is('if') ? this.ifExpression() : this.node('block', this.token.start, { body: this.body(true) })) : null;
    return this.node('if', start, { condition, body, other });
  }
  expr(min = 0) {
    if (++this.depth > 256) {
      this.depth--;
      this.report('NestingLimit', 'Expression nesting exceeds the prim-rs limit of 256 levels.', this.token);
      throw new ParseError();
    }
    try { return this.expression(min); } finally { this.depth--; }
  }
  expression(min) {
    const start = this.token.start;
    let left;
    if (['!', '-', '+'].includes(this.token.text)) {
      const op = this.take(); left = this.node('unary', start, { op, value: this.expr(60) });
    } else if (this.is('$') || (this.is('@') && this.tokens[this.pos + 1].text !== '{')) left = this.callable();
    else if (this.eat('@')) left = this.node('data', start, { body: this.body(true, true) });
    else if (this.is('{')) left = this.node('block', start, { body: this.body(true) });
    else if (this.is('if')) left = this.ifExpression();
    else if (this.eat('loop')) left = this.node('loop', start, { body: this.body(true) });
    else if (this.eat('(')) {
      if (this.eat(')')) left = this.node('unit', start);
      else { const value = this.expr(); this.expect(')'); left = this.node('group', start, { value }); }
    } else {
      const t = this.token;
      if (['name', 'int', 'float', 'str', 'invalid'].includes(t.kind) || ['true', 'false', 'self'].includes(t.text)) {
        this.take(); left = this.node(t.kind === 'keyword' ? (t.text === 'self' ? 'self' : 'bool') : t.kind, start, { token: t });
      } else {
        this.report('UnexpectedToken', 'Expected an expression.', t);
        if ([';', '}', ')', ','].includes(t.text) || t.kind === 'eof') return this.node('error', t.start, { end: t.start });
        throw new ParseError();
      }
    }
    let compared = false;
    while (true) {
      if (min <= 70 && this.is('(')) {
        const open = this.take(); const args = [], commas = [];
        const callEnd = () => [')', ';', '}'].includes(this.token.text) || this.token.kind === 'eof';
        if (!callEnd()) {
          do {
            args.push(this.argument());
            if (callEnd()) break;
            commas.push(this.expect(','));
          } while (!callEnd());
        }
        let close;
        const incomplete = !this.is(')');
        if (incomplete) {
          close = { ...this.token, end: this.token.start };
          this.report('ExpectedToken', 'Call is missing its closing ).', close);
        } else close = this.expect(')');
        left = this.node('call', start, { callee: left, args, open, close, commas, incomplete, end: close.end });
        continue;
      }
      if (min <= 70 && this.eat('.')) {
        // Keep the receiver for member completion while the field is being typed.
        let name;
        if (this.token.kind === 'name') name = this.take();
        else { name = { start: this.tokens[this.pos - 1].end, end: this.tokens[this.pos - 1].end, text: '', kind: 'name' }; this.report('ExpectedIdentifier', 'Expected a field name after the dot.', this.token); }
        left = this.node('field', start, { receiver: left, name, end: name.end }); continue;
      }
      const bp = PRECEDENCE[this.token.text];
      if (bp === undefined || bp < min) break;
      const op = this.take();
      if (bp === 30 && compared) this.report('ChainedComparison', 'Comparisons cannot be chained; use &&.', op);
      if (bp === 30) compared = true;
      const right = this.expr(bp + 1);
      left = this.node('binary', start, { left, right, op });
    }
    return left;
  }
}

module.exports = { lex, Parser, KEYWORDS };
