/*
 * lexer.re - Prim 词法分析器
 *
 * 编译命令:
 *   re2c -W -i -c -o lexer.cpp lexer.re
 *   # -W: 告警   -i: 统计   -c: 条件机
 */

#include <string>
#include <string_view>
#include <vector>
#include <stack>
#include <optional>
#include <cassert>

#include "token.hpp"
#include "macro.hpp"

namespace prim {

// ============================================================================
// Lexer - 词法分析器
// ============================================================================

class Lexer {
public:
    explicit Lexer(std::string source)
        : source_(std::move(source))
    {
        // 添加哨兵字符，简化 EOF 检查
        source_.push_back('\0');
        
        // 初始化指针
        base_   = reinterpret_cast<const unsigned char*>(source_.data());
        limit_  = base_ + source_.size();
        cursor_ = base_;
        marker_ = base_;
        
        // 初始化状态
        state_ = State::INITIAL;
        
        // 初始化位置
        current_loc_ = Location{1, 1, 0};
    }

    // 获取下一个 token
    force_inline_ Token next() {
        // 如果有 pending 的未匹配左括号错误，先返回它
        if (has_pending_bracket_error_) {
            has_pending_bracket_error_ = false;
            Token tok;
            tok.type  = TokenType::ERROR;
            tok.text  = std::string_view{};
            tok.begin = current_loc_;
            tok.end   = current_loc_;
            tok.err   = ErrType::UnmatchedLeftBracket;
            tok.emsg  = ErrMsg(pending_bracket_char_);
            return tok;
        }
        
        return scan();
    }

    // 检查是否到达文件末尾
    [[nodiscard]] bool is_eof() const {
        return cursor_ >= limit_ || *cursor_ == '\0';
    }

private:
    // ========================================================================
    // re2c 配置
    // ========================================================================
    
    /*!re2c
        re2c:api:style = free-form;
        re2c:define:YYCTYPE     = "unsigned char";
        re2c:define:YYCURSOR    = cursor_;
        re2c:define:YYMARKER    = marker_;
        re2c:define:YYLIMIT     = limit_;
        re2c:define:YYGETCONDITION = state_;
        re2c:define:YYSETCONDITION = "state_ = @@;";
        re2c:yyfill:enable      = 0;
        re2c:flags:tags         = 0;
        re2c:condprefix         = "";
        re2c:condenumprefix     = "State::";
    */

    // ========================================================================
    // 状态定义
    // ========================================================================
    
    enum class State {
        INITIAL,    // 初始状态
        STRING,     // 字符串内部
        COMMENT     // 多行注释内部
    };

    // ========================================================================
    // 成员变量
    // ========================================================================
    
    std::string              source_;           // 源代码（包含哨兵）
    const unsigned char*     base_    = nullptr; // 源代码起始指针
    const unsigned char*     limit_   = nullptr; // 源代码结束指针
    const unsigned char*     cursor_  = nullptr; // 当前扫描位置
    const unsigned char*     marker_  = nullptr; // 回溯标记
    
    State                    state_;            // 当前状态
    Location                 current_loc_;      // 当前位置
    
    const unsigned char*     token_start_ = nullptr;  // 当前 token 起始位置
    Location                 token_begin_loc_;        // 当前 token 起始 Location
    
    std::stack<char>         bracket_stack_;          // 括号栈
    bool                     has_pending_bracket_error_ = false;
    char                     pending_bracket_char_      = '\0';
    
    bool                     in_multichar_token_ = false;  // 是否在多字符token中

    // ========================================================================
    // 辅助函数
    // ========================================================================

    // 前进位置，更新 current_loc_
    force_inline_ void advance(const unsigned char* from, const unsigned char* to) {
        for (auto p = from; p < to; ++p) {
            unsigned char ch = *p;
            if (ch == '\n') {
                current_loc_.line++;
                current_loc_.col = 1;
                current_loc_.offset++;
            } else if (ch == '\r') {
                // \r 只增加 offset，不改变行列
                current_loc_.offset++;
            } else {
                current_loc_.col++;
                current_loc_.offset++;
            }
        }
    }

    // 开始一个新 token
    force_inline_ void start_token() {
        token_start_ = cursor_;
        token_begin_loc_ = current_loc_;
    }

    // 完成一个 token
    force_inline_ Token finish_token(TokenType type) {
        Token tok;
        tok.type  = type;
        tok.begin = token_begin_loc_;
        tok.end   = current_loc_;
        tok.text  = std::string_view(
            reinterpret_cast<const char*>(token_start_),
            static_cast<size_t>(cursor_ - token_start_)
        );
        tok.err   = ErrType::None;
        return tok;
    }

    // 完成一个错误 token
    force_inline_ Token finish_error(ErrType err_type, ErrMsg err_msg = ErrMsg()) {
        Token tok = finish_token(TokenType::ERROR);
        tok.err  = err_type;
        tok.emsg = err_msg;
        return tok;
    }

    // 括号匹配：左括号
    force_inline_ void push_bracket(char left) {
        bracket_stack_.push(left);
    }

    // 括号匹配：右括号
    // 返回 nullopt 表示匹配成功
    // 返回 char 表示不匹配的右括号
    force_inline_ std::optional<char> pop_bracket(char right) {
        if (bracket_stack_.empty()) {
            return right;  // 栈为空，右括号多余
        }
        
        char left = bracket_stack_.top();
        
        // 检查是否匹配
        bool matched = (left == '(' && right == ')') ||
                       (left == '[' && right == ']') ||
                       (left == '{' && right == '}');
        
        if (matched) {
            bracket_stack_.pop();
            return std::nullopt;  // 匹配成功
        } else {
            return right;  // 不匹配
        }
    }

    // 检查关键字
    force_inline_ TokenType check_keyword(std::string_view text) {
        // 使用简单的 switch + 字符串比较
        // 关键字不多，性能已经足够好
        if (text == "let")    return TokenType::KW_LET;
        if (text == "del")    return TokenType::KW_DEL;
        if (text == "if")     return TokenType::KW_IF;
        if (text == "else")   return TokenType::KW_ELSE;
        if (text == "loop")   return TokenType::KW_LOOP;
        if (text == "break")  return TokenType::KW_BREAK;
        if (text == "return") return TokenType::KW_RETURN;
        if (text == "true")   return TokenType::KW_TRUE;
        if (text == "false")  return TokenType::KW_FALSE;
        if (text == "null")   return TokenType::KW_NULL;
        return TokenType::IDENT;
    }

    // ========================================================================
    // 主扫描函数
    // ========================================================================

    force_inline_ Token scan() {
    RESTART:
        // 如果不是在多字符 token 中，开始新 token
        if (!in_multichar_token_) {
            start_token();
        }

        /*!re2c
        // ====================================================================
        // 字符类定义
        // ====================================================================
        
        WS      = [ \t\r\n]+;
        NL      = [\n] | "\r\n" | "\r";
        
        A       = [A-Za-z_];
        AN      = [A-Za-z0-9_];
        LBL     = [A-Za-z0-9_ ];
        
        D       = [0-9];
        HEX     = [0-9a-fA-F];
        OCT     = [0-7];
        BIN     = [01];
        S       = ['];

        // ====================================================================
        // INITIAL 状态 - 正常扫描
        // ====================================================================

        // --------------------------------------------------------------------
        // 空白符和注释（跳过）
        // --------------------------------------------------------------------
        
        <INITIAL> WS {
            advance(token_start_, cursor_);
            goto RESTART;
        }

        <INITIAL> "//" [^\n\r]* (NL)? {
            advance(token_start_, cursor_);
            goto RESTART;
        }

        <INITIAL> "/*" {
            advance(token_start_, cursor_);
            state_ = State::COMMENT;
            in_multichar_token_ = true;
            goto RESTART;
        }

        // --------------------------------------------------------------------
        // 双字符运算符（优先匹配）
        // --------------------------------------------------------------------
        
        <INITIAL> "==" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::EQEQ);
        }

        <INITIAL> "!=" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::NEQ);
        }

        <INITIAL> "<=" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::LE);
        }

        <INITIAL> ">=" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::GE);
        }

        <INITIAL> "&&" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::ANDAND);
        }

        <INITIAL> "||" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::OROR);
        }

        // --------------------------------------------------------------------
        // 单字符运算符
        // --------------------------------------------------------------------
        
        <INITIAL> "&" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::AMP);
        }

        <INITIAL> "|" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::PIPE);
        }

        <INITIAL> "!" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::BANG);
        }

        <INITIAL> "=" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::EQ);
        }

        <INITIAL> "+" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::PLUS);
        }

        <INITIAL> "-" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::MINUS);
        }

        <INITIAL> "*" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::STAR);
        }

        <INITIAL> "/" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::SLASH);
        }

        <INITIAL> "%" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::PERCENT);
        }

        <INITIAL> "<" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::LT);
        }

        <INITIAL> ">" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::GT);
        }

        // --------------------------------------------------------------------
        // 分隔符（括号需要匹配检查）
        // --------------------------------------------------------------------
        
        <INITIAL> "(" {
            push_bracket('(');
            advance(token_start_, cursor_);
            return finish_token(TokenType::LPAREN);
        }

        <INITIAL> ")" {
            auto mismatch = pop_bracket(')');
            advance(token_start_, cursor_);
            if (mismatch) {
                return finish_error(ErrType::UnmatchedRightBracket, ErrMsg(*mismatch));
            }
            return finish_token(TokenType::RPAREN);
        }

        <INITIAL> "[" {
            push_bracket('[');
            advance(token_start_, cursor_);
            return finish_token(TokenType::LBRACK);
        }

        <INITIAL> "]" {
            auto mismatch = pop_bracket(']');
            advance(token_start_, cursor_);
            if (mismatch) {
                return finish_error(ErrType::UnmatchedRightBracket, ErrMsg(*mismatch));
            }
            return finish_token(TokenType::RBRACK);
        }

        <INITIAL> "{" {
            push_bracket('{');
            advance(token_start_, cursor_);
            return finish_token(TokenType::LBRACE);
        }

        <INITIAL> "}" {
            auto mismatch = pop_bracket('}');
            advance(token_start_, cursor_);
            if (mismatch) {
                return finish_error(ErrType::UnmatchedRightBracket, ErrMsg(*mismatch));
            }
            return finish_token(TokenType::RBRACE);
        }

        <INITIAL> "," {
            advance(token_start_, cursor_);
            return finish_token(TokenType::COMMA);
        }

        <INITIAL> ";" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::SEMI);
        }

        <INITIAL> ":" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::COLON);
        }

        <INITIAL> "." {
            advance(token_start_, cursor_);
            return finish_token(TokenType::DOT);
        }

        <INITIAL> "@" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::AT);
        }

        <INITIAL> "$" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::DOLLAR);
        }

        // --------------------------------------------------------------------
        // 标识符和关键字
        // --------------------------------------------------------------------
        
        <INITIAL> A AN* {
            advance(token_start_, cursor_);
            std::string_view text(
                reinterpret_cast<const char*>(token_start_),
                cursor_ - token_start_
            );
            TokenType type = check_keyword(text);
            return finish_token(type);
        }

        // --------------------------------------------------------------------
        // 数字字面量
        // --------------------------------------------------------------------
        
        DEC_G   = D+ (S D+)*;
        HEX_G   = HEX+ (S HEX+)*;
        OCT_G   = OCT+ (S OCT+)*;
        BIN_G   = BIN+ (S BIN+)*;
        EXP     = [eE] [+\-]? DEC_G;

        // 十六进制整数
        <INITIAL> ("0x" | "0X") HEX_G {
            advance(token_start_, cursor_);
            return finish_token(TokenType::INT_HEX);
        }

        // 八进制整数
        <INITIAL> ("0o" | "0O") OCT_G {
            advance(token_start_, cursor_);
            return finish_token(TokenType::INT_OCT);
        }

        // 二进制整数
        <INITIAL> ("0b" | "0B") BIN_G {
            advance(token_start_, cursor_);
            return finish_token(TokenType::INT_BIN);
        }

        // 十进制整数
        <INITIAL> DEC_G {
            advance(token_start_, cursor_);
            return finish_token(TokenType::INT_DEC);
        }

        // 浮点数（四种形式）
        <INITIAL> DEC_G "." DEC_G (EXP)? {
            advance(token_start_, cursor_);
            return finish_token(TokenType::FLOAT_DEC);
        }

        <INITIAL> DEC_G "." (EXP)? {
            advance(token_start_, cursor_);
            return finish_token(TokenType::FLOAT_DEC);
        }

        <INITIAL> "." DEC_G (EXP)? {
            advance(token_start_, cursor_);
            return finish_token(TokenType::FLOAT_DEC);
        }

        <INITIAL> DEC_G EXP {
            advance(token_start_, cursor_);
            return finish_token(TokenType::FLOAT_DEC);
        }

        // --------------------------------------------------------------------
        // 数字字面量错误
        // --------------------------------------------------------------------

        // 前缀后非法字符
        <INITIAL> ("0x" | "0X") [^0-9a-fA-F'] {
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalNumber, ErrMsg(2));
        }

        <INITIAL> ("0o" | "0O") [^0-7'] {
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalNumber, ErrMsg(2));
        }

        <INITIAL> ("0b" | "0B") [^01'] {
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalNumber, ErrMsg(2));
        }

        // 分隔符误用：以 ' 开头
        <INITIAL> S D+ {
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalNumber, ErrMsg(0));
        }

        // 连续分隔符：1''2
        <INITIAL> D+ S S D* {
            // 找到第一个 '' 的位置
            const unsigned char* p = token_start_;
            int pos = 0;
            while (p + 1 < cursor_) {
                if (*p == '\'' && *(p + 1) == '\'') {
                    pos = int(p - token_start_);
                    break;
                }
                ++p;
            }
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalNumber, ErrMsg(pos));
        }

        // 末尾分隔符：123' (后跟空白或分隔符)
        <INITIAL> D+ S / [ \t\r\n;,)\]}] {
            // 找到 ' 的位置
            const unsigned char* p = cursor_ - 1;
            while (p >= token_start_ && *p != '\'') --p;
            int pos = int(p - token_start_);
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalNumber, ErrMsg(pos));
        }

        // 小数点后非数字
        <INITIAL> DEC_G "." [^0-9eE] {
            int pos = int(cursor_ - token_start_) - 2;  // 指向 '.'
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalNumber, ErrMsg(pos));
        }

        // 指数后缺数字
        <INITIAL> DEC_G [eE] [+\-]? [^0-9'] {
            int pos = int(cursor_ - token_start_) - 1;
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalNumber, ErrMsg(pos));
        }

        // "." 后跟 "'"
        <INITIAL> "." S {
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalNumber, ErrMsg(0));
        }

        // --------------------------------------------------------------------
        // 字符串字面量
        // --------------------------------------------------------------------
        
        <INITIAL> "\"" {
            advance(token_start_, cursor_);
            state_ = State::STRING;
            in_multichar_token_ = true;
            goto RESTART;
        }

        // --------------------------------------------------------------------
        // 标签
        // --------------------------------------------------------------------
        
        // 合法标签：`label`
        <INITIAL> "`" A LBL* "`" {
            advance(token_start_, cursor_);
            return finish_token(TokenType::LABEL);
        }

        // 空标签：``
        <INITIAL> "``" {
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalLabel, ErrMsg(1));
        }

        // 非字母开头：`123`
        <INITIAL> "`" [^A-Za-z_\n`] LBL* "`" {
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalLabel, ErrMsg(1));
        }

        // 包含非法字符
        <INITIAL> "`" A LBL* [^A-Za-z0-9_ \n`] [^`\n]* "`" {
            // 找到第一个非法字符位置
            const unsigned char* p = token_start_ + 1;
            int pos = 1;
            while (p < cursor_ - 1) {
                unsigned char c = *p;
                bool valid = (c == ' ' || c == '_' ||
                             (c >= '0' && c <= '9') ||
                             (c >= 'A' && c <= 'Z') ||
                             (c >= 'a' && c <= 'z'));
                if (!valid) {
                    pos = int(p - token_start_);
                    break;
                }
                ++p;
            }
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalLabel, ErrMsg(pos));
        }

        // 未闭合标签
        <INITIAL> "`" [^\n`]* {
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalLabel, ErrMsg(0));
        }

        // EOF 和非法字符
        // --------------------------------------------------------------------
        
        <INITIAL> "\000" {
            // 检查是否有未匹配的左括号
            if (!bracket_stack_.empty()) {
                pending_bracket_char_ = bracket_stack_.top();
                bracket_stack_ = std::stack<char>();  // 清空栈
                has_pending_bracket_error_ = true;
            }
            advance(token_start_, cursor_);
            return finish_token(TokenType::END);
        }

        <INITIAL> . {
            char illegal_char = *(cursor_ - 1);
            advance(token_start_, cursor_);
            return finish_error(ErrType::IllegalChar, ErrMsg(illegal_char));
        }

        // ====================================================================
        // STRING 状态 - 字符串内部
        // ====================================================================

        // 闭合引号
        <STRING> "\"" {
            advance(token_start_, cursor_);
            state_ = State::INITIAL;
            in_multichar_token_ = false;
            return finish_token(TokenType::STRING);
        }

        // 转义序列
        <STRING> "\\\""  { advance(token_start_, cursor_); goto RESTART; }
        <STRING> "\\\\"  { advance(token_start_, cursor_); goto RESTART; }
        <STRING> "\\'"   { advance(token_start_, cursor_); goto RESTART; }
        <STRING> "\\n"   { advance(token_start_, cursor_); goto RESTART; }
        <STRING> "\\r"   { advance(token_start_, cursor_); goto RESTART; }
        <STRING> "\\t"   { advance(token_start_, cursor_); goto RESTART; }
        <STRING> "\\0"   { advance(token_start_, cursor_); goto RESTART; }
        <STRING> "\\x" HEX+       { advance(token_start_, cursor_); goto RESTART; }
        <STRING> "\\" OCT{1,3}    { advance(token_start_, cursor_); goto RESTART; }
        <STRING> "\\u" HEX{4}     { advance(token_start_, cursor_); goto RESTART; }
        <STRING> "\\U" HEX{8}     { advance(token_start_, cursor_); goto RESTART; }

        // 普通字符
        <STRING> [^"\\\n\r\000]+ { advance(token_start_, cursor_); goto RESTART; }

        // 非法转义或未闭合
        <STRING> "\\" (NL | "\000") {
            advance(token_start_, cursor_);
            state_ = State::INITIAL;
            in_multichar_token_ = false;
            return finish_error(ErrType::UnterminatedString);
        }

        <STRING> "\\" . {
            // 反斜杠位置
            int pos = int(cursor_ - token_start_) - 2;
            advance(token_start_, cursor_);
            state_ = State::INITIAL;
            in_multichar_token_ = false;
            return finish_error(ErrType::IllegalEscape, ErrMsg(pos));
        }

        <STRING> (NL | "\000") {
            advance(token_start_, cursor_);
            state_ = State::INITIAL;
            in_multichar_token_ = false;
            return finish_error(ErrType::UnterminatedString);
        }

        // ====================================================================
        // COMMENT 状态 - 多行注释内部
        // ====================================================================

        // 注释结束
        <COMMENT> "*/" {
            advance(token_start_, cursor_);
            state_ = State::INITIAL;
            in_multichar_token_ = false;
            goto RESTART;
        }

        // 换行
        <COMMENT> NL {
            advance(token_start_, cursor_);
            goto RESTART;
        }

        // EOF - 未闭合注释
        <COMMENT> "\000" {
            advance(token_start_, cursor_);
            state_ = State::INITIAL;
            in_multichar_token_ = false;
            return finish_error(ErrType::UnterminatedComment);
        }

        // 普通字符（包括 * 但不跟 /）
        <COMMENT> [^*\n\r\000]+ { advance(token_start_, cursor_); goto RESTART; }
        <COMMENT> "*"           { advance(token_start_, cursor_); goto RESTART; }

        */

        // 不应该到达这里
        assert(false && "Lexer: unreachable code");
        return finish_token(TokenType::END);
    }
};

} // namespace prim
