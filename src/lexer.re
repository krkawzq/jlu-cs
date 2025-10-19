/*
 * lexer.re
 *
 *   re2c -W -i -c -o lexer.cpp lexer.re
 *   # -W: 告警   -i: 统计   -c: 条件机
 */

#include <cstdint>
#include <string>
#include <string_view>
#include <vector>
#include <optional>

struct Location {
    int   line   = 1;
    int   col    = 1;      // 1-based
    int   offset = 0;      // 0-based
};

enum class TokenKind : uint16_t {
    // 关键字
    KW_LET, KW_DEL, KW_IF, KW_ELSE, KW_LOOP, KW_BREAK, KW_RETURN, KW_TRUE, KW_FALSE, KW_NULL,

    // 标识符 / 字面量
    IDENT,
    INT_DEC, INT_HEX, INT_OCT, INT_BIN,
    FLOAT_DEC,
    STRING,
    LABEL,  // `label_name`

    // 运算符与符号
    AMP, BANG,
    PLUS, MINUS, STAR, SLASH, PERCENT,
    EQ, EQEQ, NEQ, LT, GT, LE, GE,
    ANDAND, OROR,

    LPAREN, RPAREN, LBRACK, RBRACK, LBRACE, RBRACE,
    AT, DOLLAR, DOT, COMMA, SEMI, COLON,

    END,
    ERROR
};

enum class ErrKind : uint16_t {
    None = 0,
    IllegalChar,
    UnterminatedString,
    UnterminatedComment,
    UnmatchedLeftBracket,   // EOF 时栈未空
    UnmatchedRightBracket,  // 读到不匹配右括号
    IllegalNumber,          // 非法数字（union.pos）
    IllegalEscape,          // 非法转义（union.pos）
    IllegalLabel,           // 标签错误（union.pos）
    IllegalIdentifier       // 预留
};

union ErrMsg {
    int pos;  // 数字/转义/标签：相对 token 起始 0-based 位置
    int ch;   // 括号：相关括号的 ASCII
};

struct Token {
    TokenKind           kind;
    std::string_view    lexeme;
    Location            begin;
    Location            end;
    ErrKind             err   = ErrKind::None;
    ErrMsg              emsg  { .pos = -1 };
};

class Lexer {
public:
    explicit Lexer(std::string input)
        : storage_(std::move(input))
    {
        storage_.push_back('\0'); // 哨兵
        base_   = reinterpret_cast<const unsigned char*>(storage_.data());
        limit_  = base_ + storage_.size();
        cursor_ = base_;
        marker_ = base_;
        cond_   = Condition::INITIAL;
        loc_    = {};
        loc_.line = 1; loc_.col = 1; loc_.offset = 0;
    }

    Token next() {
        if (pending_unmatched_left_) {
            pending_unmatched_left_ = false;
            Token t;
            t.kind  = TokenKind::ERROR;
            t.lexeme= std::string_view{};
            t.begin = loc_;
            t.end   = loc_;
            t.err   = ErrKind::UnmatchedLeftBracket;
            t.emsg.ch = last_unclosed_left_;
            return t;
        }
        return lex_impl_();
    }

private:
    /*!re2c
        re2c:api:style = free-form;
        re2c:define:YYCTYPE  = "unsigned char";
        re2c:define:YYCURSOR = cursor_;
        re2c:define:YYMARKER = marker_;
        re2c:define:YYLIMIT  = limit_;
        re2c:define:YYGETCONDITION = cond_;
        re2c:define:YYSETCONDITION = "cond_ = @@;";
        re2c:yyfill:enable   = 0;
        re2c:flags:tags      = 0;
        re2c:condprefix = "";
        re2c:condenumprefix = "Condition::";
    */

    enum class Condition { INITIAL, STRING, COMMENT };
    Condition cond_;

    std::string              storage_;
    const unsigned char*     base_   = nullptr;
    const unsigned char*     limit_  = nullptr;
    const unsigned char*     cursor_ = nullptr;
    const unsigned char*     marker_ = nullptr;

    Location                 loc_{};
    const unsigned char*     tok_begin_ = nullptr;
    Location                 begin_loc_{}, end_loc_{};

    std::vector<char>        br_stack_{};
    bool                     pending_unmatched_left_ = false;
    int                      last_unclosed_left_     = 0;

    bool                     in_multi_char_token_    = false;

    void advance_span_(const unsigned char* from, const unsigned char* to) {
        for (auto p = from; p < to; ++p) {
            unsigned char c = *p;
            if (c == '\n') { loc_.line++; loc_.col = 1; loc_.offset++; }
            else if (c == '\r') { loc_.offset++; }
            else { loc_.col++; loc_.offset++; }
        }
    }

    void start_token_() {
        tok_begin_ = cursor_;
        begin_loc_ = loc_;
    }
    Token finish_token_(TokenKind k) {
        end_loc_ = loc_;
        Token t;
        t.kind   = k;
        t.begin  = begin_loc_;
        t.end    = end_loc_;
        t.lexeme = std::string_view(
            reinterpret_cast<const char*>(tok_begin_),
            static_cast<size_t>(cursor_ - tok_begin_));
        return t;
    }
    Token finish_error_(ErrKind ek, ErrMsg em = {.pos=-1}) {
        Token t = finish_token_(TokenKind::ERROR);
        t.err   = ek;
        t.emsg  = em;
        return t;
    }

    void push_br_(char c) { br_stack_.push_back(c); }
    std::optional<char> pop_br_if_match_(char right) {
        if (br_stack_.empty()) return right;
        char left = br_stack_.back();
        if ((left=='(' && right==')') ||
            (left=='[' && right==']') ||
            (left=='{' && right=='}')) {
            br_stack_.pop_back();
            return std::nullopt;
        }
        return right;
    }

    Token lex_impl_() {
        // —— 关键：消除递归。用本地循环重启扫描 —— //
    RESTART:
        if (!in_multi_char_token_) {
            start_token_();
        }

        /*!re2c

        WS      = [ \t\r\n]+;
        NL      = [\n] | "\r\n" | "\r";
        A       = [A-Za-z_];
        AN      = [A-Za-z0-9_];
        LBL     = [A-Za-z0-9_ ];
        D       = [0-9];
        S       = ['];
        HEX     = [0-9a-fA-F];
        OCT     = [0-7];
        BIN     = [01];

        // ==== 运算符（双字符优先） ====
        <INITIAL> "=="  { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::EQEQ);  }
        <INITIAL> "!="  { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::NEQ);   }
        <INITIAL> "<="  { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::LE);    }
        <INITIAL> ">="  { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::GE);    }
        <INITIAL> "&&"  { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::ANDAND);}
        <INITIAL> "||"  { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::OROR);  }

        <INITIAL> "&"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::AMP);   }
        <INITIAL> "!"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::BANG);  }
        <INITIAL> "="   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::EQ);    }
        <INITIAL> "+"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::PLUS);  }
        <INITIAL> "-"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::MINUS); }
        <INITIAL> "*"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::STAR);  }
        <INITIAL> "/"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::SLASH); }
        <INITIAL> "%"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::PERCENT);}
        <INITIAL> "<"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::LT);    }
        <INITIAL> ">"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::GT);    }
        <INITIAL> "("   { advance_span_(tok_begin_, cursor_); push_br_('('); return finish_token_(TokenKind::LPAREN); }
        <INITIAL> ")"   {
                            auto mis = pop_br_if_match_(')');
                            advance_span_(tok_begin_, cursor_);
                            if (mis) { ErrMsg em; em.ch = *mis; return finish_error_(ErrKind::UnmatchedRightBracket, em); }
                            return finish_token_(TokenKind::RPAREN);
                        }
        <INITIAL> "["   { advance_span_(tok_begin_, cursor_); push_br_('['); return finish_token_(TokenKind::LBRACK); }
        <INITIAL> "]"   {
                            auto mis = pop_br_if_match_(']');
                            advance_span_(tok_begin_, cursor_);
                            if (mis) { ErrMsg em; em.ch = *mis; return finish_error_(ErrKind::UnmatchedRightBracket, em); }
                            return finish_token_(TokenKind::RBRACK);
                        }
        <INITIAL> "{"   { advance_span_(tok_begin_, cursor_); push_br_('{'); return finish_token_(TokenKind::LBRACE); }
        <INITIAL> "}"   {
                            auto mis = pop_br_if_match_('}');
                            advance_span_(tok_begin_, cursor_);
                            if (mis) { ErrMsg em; em.ch = *mis; return finish_error_(ErrKind::UnmatchedRightBracket, em); }
                            return finish_token_(TokenKind::RBRACE);
                        }
        <INITIAL> "@"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::AT);    }
        <INITIAL> "$"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::DOLLAR);}
        <INITIAL> "."   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::DOT);   }
        <INITIAL> ","   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::COMMA); }
        <INITIAL> ";"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::SEMI);  }
        <INITIAL> ":"   { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::COLON); }

        // ==== 注释与空白（改为 goto RESTART，避免递归） ====
        <INITIAL> WS   { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <INITIAL> "//" [^\n\r]* ( NL )? { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <INITIAL> "/*"  {
            advance_span_(tok_begin_, cursor_);
            cond_ = Condition::COMMENT;
            in_multi_char_token_ = true;
            goto RESTART;
        }

        // ==== 字符串 ====
        <INITIAL> "\""  {
            advance_span_(tok_begin_, cursor_);
            cond_ = Condition::STRING;
            in_multi_char_token_ = true;
            goto RESTART;
        }

        // ==== 标签 ====
        <INITIAL> "`" A LBL* "`" {
            advance_span_(tok_begin_, cursor_);
            return finish_token_(TokenKind::LABEL);
        }
        <INITIAL> "``" {
            ErrMsg em; em.pos = 1;
            advance_span_(tok_begin_, cursor_);
            return finish_error_(ErrKind::IllegalLabel, em);
        }
        <INITIAL> "`" [^A-Za-z_\n`] LBL* "`" {
            ErrMsg em; em.pos = 1;
            advance_span_(tok_begin_, cursor_);
            return finish_error_(ErrKind::IllegalLabel, em);
        }
        <INITIAL> "`" A LBL* [^A-Za-z0-9_ \n`] [^`\n]* "`" {
            // 第一个非法字符位置
            const unsigned char* p = tok_begin_ + 1;
            int pos = 1;
            while (p < cursor_ - 1) {
                unsigned char c = *p;
                bool ok = (c==' ') || (c=='_') ||
                          (c>='0'&&c<='9') || (c>='A'&&c<='Z') || (c>='a'&&c<='z');
                if (!ok) { pos = int(p - tok_begin_); break; }
                ++p;
            }
            ErrMsg em; em.pos = pos;
            advance_span_(tok_begin_, cursor_);
            return finish_error_(ErrKind::IllegalLabel, em);
        }
        <INITIAL> "`" [^\n`]* {
            ErrMsg em; em.pos = 0;
            advance_span_(tok_begin_, cursor_);
            return finish_error_(ErrKind::IllegalLabel, em);
        }

        // ==== 关键字 / 标识符 ====
        <INITIAL> A AN* {
            advance_span_(tok_begin_, cursor_);
            std::string_view s(reinterpret_cast<const char*>(tok_begin_), cursor_ - tok_begin_);
            TokenKind k = TokenKind::IDENT;
            if      (s == "let")    k = TokenKind::KW_LET;
            else if (s == "del")    k = TokenKind::KW_DEL;
            else if (s == "if")     k = TokenKind::KW_IF;
            else if (s == "else")   k = TokenKind::KW_ELSE;
            else if (s == "loop")   k = TokenKind::KW_LOOP;
            else if (s == "break")  k = TokenKind::KW_BREAK;
            else if (s == "return") k = TokenKind::KW_RETURN;
            else if (s == "true")   k = TokenKind::KW_TRUE;
            else if (s == "false")  k = TokenKind::KW_FALSE;
            else if (s == "null")   k = TokenKind::KW_NULL;
            return finish_token_(k);
        }

        // ==== 数字 ====
        DEC_G   = D ( S D )*;
        HEX_G   = HEX ( S HEX )*;
        OCT_G   = OCT ( S OCT )*;
        BIN_G   = BIN ( S BIN )*;
        EXP     = [eE] [+\-]? DEC_G;

        // 整数
        <INITIAL> "0x" HEX_G { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::INT_HEX); }
        <INITIAL> "0X" HEX_G { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::INT_HEX); }
        <INITIAL> "0o" OCT_G { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::INT_OCT); }
        <INITIAL> "0O" OCT_G { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::INT_OCT); }
        <INITIAL> "0b" BIN_G { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::INT_BIN); }
        <INITIAL> "0B" BIN_G { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::INT_BIN); }
        <INITIAL> DEC_G      { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::INT_DEC); }

        // 浮点（四形态）
        <INITIAL> DEC_G "." DEC_G ( EXP )? { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::FLOAT_DEC); }
        <INITIAL> DEC_G "." ( EXP )?        { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::FLOAT_DEC); }
        <INITIAL> "." DEC_G ( EXP )?        { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::FLOAT_DEC); }
        <INITIAL> DEC_G EXP                 { advance_span_(tok_begin_, cursor_); return finish_token_(TokenKind::FLOAT_DEC); }

        // —— 非法数字（更精准的特化）——
        // 0x/0o/0b 前缀后非合法基数
        <INITIAL> "0x" [^0-9a-fA-F] { ErrMsg em; em.pos = 2; advance_span_(tok_begin_, cursor_); return finish_error_(ErrKind::IllegalNumber, em); }
        <INITIAL> "0X" [^0-9a-fA-F] { ErrMsg em; em.pos = 2; advance_span_(tok_begin_, cursor_); return finish_error_(ErrKind::IllegalNumber, em); }
        <INITIAL> "0o" [^0-7]       { ErrMsg em; em.pos = 2; advance_span_(tok_begin_, cursor_); return finish_error_(ErrKind::IllegalNumber, em); }
        <INITIAL> "0O" [^0-7]       { ErrMsg em; em.pos = 2; advance_span_(tok_begin_, cursor_); return finish_error_(ErrKind::IllegalNumber, em); }
        <INITIAL> "0b" [^01]        { ErrMsg em; em.pos = 2; advance_span_(tok_begin_, cursor_); return finish_error_(ErrKind::IllegalNumber, em); }
        <INITIAL> "0B" [^01]        { ErrMsg em; em.pos = 2; advance_span_(tok_begin_, cursor_); return finish_error_(ErrKind::IllegalNumber, em); }

        // 单引号误用：以 ' 开头
        <INITIAL> S D+            { ErrMsg em; em.pos = 0; advance_span_(tok_begin_, cursor_); return finish_error_(ErrKind::IllegalNumber, em); }
        // 连续单引号：1''2
        <INITIAL> D+ S S [0-9]*  {
            const unsigned char* p = tok_begin_;
            int pos = 0;
            while (p + 1 < cursor_) { if (*p=='\'' && *(p+1)=='\'') { pos = int(p - tok_begin_); break; } ++p; }
            ErrMsg em; em.pos = pos;
            advance_span_(tok_begin_, cursor_);
            return finish_error_(ErrKind::IllegalNumber, em);
        }
        // 末尾单引号：123' + 分隔/结束
        <INITIAL> D+ S / [ \t\r\n;,)\]}"] {
            const unsigned char* p = tok_begin_;
            int pos = 0;
            while (p < cursor_) { if (*p=='\'') { pos = int(p - tok_begin_); break; } ++p; }
            ErrMsg em; em.pos = pos;
            advance_span_(tok_begin_, cursor_);
            return finish_error_(ErrKind::IllegalNumber, em);
        }

        // 小数点后非数字：123.[^0-9]
        <INITIAL> DEC_G "." [^0-9] {
            ErrMsg em; em.pos = int((cursor_ - tok_begin_) - 1); // 指向 '.'
            advance_span_(tok_begin_, cursor_);
            return finish_error_(ErrKind::IllegalNumber, em);
        }
        // 指数后缺数字：123e(+|-)?
        <INITIAL> DEC_G [eE] [+\-]? [^0-9] {
            // 报错位置：e 或 e+/e- 后的那个非数字
            ErrMsg em; em.pos = int((cursor_ - tok_begin_) - 1);
            advance_span_(tok_begin_, cursor_);
            return finish_error_(ErrKind::IllegalNumber, em);
        }
        // "." 后直接跟 "'"：."'
        <INITIAL> "." S {
            ErrMsg em; em.pos = 0;
            advance_span_(tok_begin_, cursor_);
            return finish_error_(ErrKind::IllegalNumber, em);
        }

        // 注：原有兜底规则（[0-9] 和 "."）会被前面更具体的规则覆盖，因此删除

        // ==== STRING 条件 ====
        <STRING> "\"" {
            advance_span_(tok_begin_, cursor_);
            cond_ = Condition::INITIAL;
            in_multi_char_token_ = false;
            return finish_token_(TokenKind::STRING);
        }
        <STRING> "\\\""  { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> "\\\\"  { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> "\\'"  { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> "\\n"   { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> "\\r"   { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> "\\t"   { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> "\\0"   { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> "\\x" HEX+    { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> "\\" OCT{1,3} { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> "\\u" HEX{4}  { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> "\\U" HEX{8}  { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> [^"\\\n\r]+    { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <STRING> "\\" ( NL | "\000" ) {
            // 反斜杠+换行或EOF：报错未闭合字符串
            advance_span_(tok_begin_, cursor_);
            cond_ = Condition::INITIAL;
            in_multi_char_token_ = false;
            return finish_error_(ErrKind::UnterminatedString);
        }
        <STRING> "\\" [^\n\r\000] {
            // 非法转义字符（排除换行和EOF）
            ErrMsg em; em.pos = int((cursor_ - tok_begin_) - 2);
            advance_span_(tok_begin_, cursor_);
            cond_ = Condition::INITIAL;
            in_multi_char_token_ = false;
            return finish_error_(ErrKind::IllegalEscape, em);
        }
        <STRING> NL | "\000" {
            advance_span_(tok_begin_, cursor_);
            cond_ = Condition::INITIAL;
            in_multi_char_token_ = false;
            return finish_error_(ErrKind::UnterminatedString);
        }

        // ==== COMMENT 条件 ====
        <COMMENT> "*/" {
            advance_span_(tok_begin_, cursor_);
            cond_ = Condition::INITIAL;
            in_multi_char_token_ = false;
            goto RESTART;
        }
        <COMMENT> NL         { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <COMMENT> "\000" {
            advance_span_(tok_begin_, cursor_);
            cond_ = Condition::INITIAL;
            in_multi_char_token_ = false;
            return finish_error_(ErrKind::UnterminatedComment);
        }
        <COMMENT> [^*\n\r\000]+ { advance_span_(tok_begin_, cursor_); goto RESTART; }
        <COMMENT> "*"           { advance_span_(tok_begin_, cursor_); goto RESTART; }

        // ==== EOF / 非法字符 ====
        <INITIAL> "\000" {
            if (!br_stack_.empty()) {
                last_unclosed_left_   = br_stack_.back();
                br_stack_.clear();
                pending_unmatched_left_ = true;
            }
            advance_span_(tok_begin_, cursor_);
            return finish_token_(TokenKind::END);
        }

        <INITIAL> . {
            ErrMsg em; em.ch = int(*(cursor_ - 1));
            advance_span_(tok_begin_, cursor_);
            return finish_error_(ErrKind::IllegalChar, em);
        }
        */

        // 防御返回
        return finish_token_(TokenKind::END);
    }
};
