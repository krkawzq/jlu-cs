// token.hpp - Prim 语言 Token 定义
#pragma once

#include <cstdint>
#include <string_view>
#include <array>
#include <ostream>

namespace prim {

// ============================================================================
// Location - 源码位置信息
// ============================================================================

struct Location {
    int line   = 1;  // 行号 (1-based)
    int col    = 1;  // 列号 (1-based)
    int offset = 0;  // 文件偏移 (0-based)
    
    constexpr Location() = default;
    constexpr Location(int l, int c, int o) : line(l), col(c), offset(o) {}
};

// operator<< for Bison trace output
inline std::ostream& operator<<(std::ostream& os, const Location& loc) {
    return os << loc.line << ":" << loc.col;
}

// ============================================================================
// TokenType - Token 类型枚举
// ============================================================================

enum class TokenType : uint16_t {
    // === 特殊 Token ===
    START,    // 初始状态标记
    END,      // 文件结束 (EOF)
    ERROR,    // 错误 token
    
    // === 关键字 (Keywords) ===
    KW_LET,      // let
    KW_DEL,      // del
    KW_IF,       // if
    KW_ELSE,     // else
    KW_LOOP,     // loop
    KW_BREAK,    // break
    KW_RETURN,   // return
    KW_TRUE,     // true
    KW_FALSE,    // false
    KW_NULL,     // null
    
    // === 标识符与字面量 (Identifiers & Literals) ===
    IDENT,       // 标识符 (identifier)
    
    // 整数字面量
    INT_DEC,     // 十进制整数
    INT_HEX,     // 十六进制整数 (0x/0X)
    INT_OCT,     // 八进制整数 (0o/0O)
    INT_BIN,     // 二进制整数 (0b/0B)
    
    // 浮点数字面量
    FLOAT_DEC,   // 十进制浮点数
    
    // 其他字面量
    STRING,      // 字符串字面量
    LABEL,       // 标签 (`label`)
    
    // === 运算符 (Operators) ===
    
    // 单字符运算符
    AMP,         // &  (引用/按位与)
    BANG,        // !  (逻辑非)
    PLUS,        // +  (加法/一元正号)
    MINUS,       // -  (减法/一元负号)
    STAR,        // *  (乘法)
    SLASH,       // /  (除法)
    PERCENT,     // %  (取模)
    LT,          // <  (小于)
    GT,          // >  (大于)
    EQ,          // =  (赋值)
    PIPE,        // |  (类型联合)
    
    // 双字符运算符
    EQEQ,        // == (相等)
    NEQ,         // != (不等)
    LE,          // <= (小于等于)
    GE,          // >= (大于等于)
    ANDAND,      // && (逻辑与)
    OROR,        // || (逻辑或)
    
    // === 分隔符 (Delimiters) ===
    LPAREN,      // (  左圆括号
    RPAREN,      // )  右圆括号
    LBRACK,      // [  左方括号
    RBRACK,      // ]  右方括号
    LBRACE,      // {  左花括号
    RBRACE,      // }  右花括号
    
    COMMA,       // ,  逗号
    SEMI,        // ;  分号
    COLON,       // :  冒号
    DOT,         // .  点号 (成员访问)
    AT,          // @  at符号 (装饰器/闭包)
    DOLLAR,      // $  美元符号 (函数定义)
};

// ============================================================================
// ErrType - 词法错误类型
// ============================================================================

enum class ErrType : uint16_t {
    None = 0,               // 无错误
    
    // 字符相关错误
    IllegalChar,            // 非法字符 (emsg.ch: 字符ASCII码)
    IllegalIdentifier,      // 非法标识符 (预留)
    
    // 字符串相关错误
    UnterminatedString,     // 未闭合字符串
    IllegalEscape,          // 非法转义序列 (emsg.pos: 反斜杠位置)
    
    // 注释相关错误
    UnterminatedComment,    // 未闭合多行注释
    
    // 括号匹配错误
    UnmatchedLeftBracket,   // 未匹配的左括号 (EOF时栈非空, emsg.ch: 括号字符)
    UnmatchedRightBracket,  // 未匹配的右括号 (emsg.ch: 括号字符)
    
    // 字面量相关错误
    IllegalNumber,          // 非法数字字面量 (emsg.pos: 错误位置)
    IllegalLabel,           // 非法标签 (emsg.pos: 错误位置)
};

// ============================================================================
// ErrMsg - 错误附加信息
// ============================================================================

union ErrMsg {
    int pos;   // 位置信息：相对token起始的0-based偏移
    char ch;    // 字符信息：ASCII码值
    
    constexpr ErrMsg() : pos(-1) {}
    constexpr explicit ErrMsg(int p) : pos(p) {}
    constexpr explicit ErrMsg(char c) : ch(c) {}
};

// ============================================================================
// Token - 词法单元
// ============================================================================

struct Token {
    TokenType        type   = TokenType::START;  // Token类型
    std::string_view text;                      // 词素（源码中的原始文本）
    Location         begin;                      // 起始位置
    Location         end;                        // 结束位置
    ErrType          err;                        // 错误类型
    ErrMsg           emsg;                       // 错误附加信息
    
    // 构造函数
    constexpr Token() = default;
    
    constexpr Token(TokenType k, std::string_view lex, Location b, Location e)
        : type(k), text(lex), begin(b), end(e), err(ErrType::None), emsg() {}
    
    constexpr Token(TokenType k, std::string_view lex, Location b, Location e, 
                    ErrType et, ErrMsg em)
        : type(k), text(lex), begin(b), end(e), err(et), emsg(em) {}
    
    // 判断是否为错误 token
    [[nodiscard]] constexpr bool is_error() const noexcept {
        return type == TokenType::ERROR || err != ErrType::None;
    }
    
    // 判断是否为EOF
    [[nodiscard]] constexpr bool is_eof() const noexcept {
        return type == TokenType::END;
    }
    
    // 判断是否为关键字
    [[nodiscard]] constexpr bool is_keyword() const noexcept {
        return type >= TokenType::KW_LET && type <= TokenType::KW_NULL;
    }
    
    // 判断是否为字面量
    [[nodiscard]] constexpr bool is_literal() const noexcept {
        return (type >= TokenType::INT_DEC && type <= TokenType::STRING) ||
               type == TokenType::KW_TRUE || 
               type == TokenType::KW_FALSE || 
               type == TokenType::KW_NULL;
    }
    
    // 判断是否为运算符
    [[nodiscard]] constexpr bool is_operator() const noexcept {
        return type >= TokenType::AMP && type <= TokenType::OROR;
    }
};

// ============================================================================
// Token 辅助函数
// ============================================================================

// Token 类型名称（用于调试和错误信息）
constexpr const char* token_type_name(TokenType kind) {
    switch (kind) {
        case TokenType::START:       return "START";
        case TokenType::END:         return "END";
        case TokenType::ERROR:       return "ERROR";
        
        case TokenType::KW_LET:      return "let";
        case TokenType::KW_DEL:      return "del";
        case TokenType::KW_IF:       return "if";
        case TokenType::KW_ELSE:     return "else";
        case TokenType::KW_LOOP:     return "loop";
        case TokenType::KW_BREAK:    return "break";
        case TokenType::KW_RETURN:   return "return";
        case TokenType::KW_TRUE:     return "true";
        case TokenType::KW_FALSE:    return "false";
        case TokenType::KW_NULL:     return "null";
        
        case TokenType::IDENT:       return "IDENT";
        case TokenType::INT_DEC:     return "INT_DEC";
        case TokenType::INT_HEX:     return "INT_HEX";
        case TokenType::INT_OCT:     return "INT_OCT";
        case TokenType::INT_BIN:     return "INT_BIN";
        case TokenType::FLOAT_DEC:   return "FLOAT_DEC";
        case TokenType::STRING:      return "STRING";
        case TokenType::LABEL:       return "LABEL";
        
        case TokenType::AMP:         return "&";
        case TokenType::BANG:        return "!";
        case TokenType::PLUS:        return "+";
        case TokenType::MINUS:       return "-";
        case TokenType::STAR:        return "*";
        case TokenType::SLASH:       return "/";
        case TokenType::PERCENT:     return "%";
        case TokenType::LT:          return "<";
        case TokenType::GT:          return ">";
        case TokenType::EQ:          return "=";
        case TokenType::PIPE:        return "|";
        case TokenType::EQEQ:        return "==";
        case TokenType::NEQ:         return "!=";
        case TokenType::LE:          return "<=";
        case TokenType::GE:          return ">=";
        case TokenType::ANDAND:      return "&&";
        case TokenType::OROR:        return "||";
        
        case TokenType::LPAREN:      return "(";
        case TokenType::RPAREN:      return ")";
        case TokenType::LBRACK:      return "[";
        case TokenType::RBRACK:      return "]";
        case TokenType::LBRACE:      return "{";
        case TokenType::RBRACE:      return "}";
        case TokenType::COMMA:       return ",";
        case TokenType::SEMI:        return ";";
        case TokenType::COLON:       return ":";
        case TokenType::DOT:         return ".";
        case TokenType::AT:          return "@";
        case TokenType::DOLLAR:      return "$";
        
        default:                     return "<UNKNOWN>";
    }
}

// 错误类型名称
constexpr const char* err_type_name(ErrType kind) {
    switch (kind) {
        case ErrType::None:                    return "None";
        case ErrType::IllegalChar:             return "IllegalChar";
        case ErrType::IllegalIdentifier:       return "IllegalIdentifier";
        case ErrType::UnterminatedString:      return "UnterminatedString";
        case ErrType::IllegalEscape:           return "IllegalEscape";
        case ErrType::UnterminatedComment:     return "UnterminatedComment";
        case ErrType::UnmatchedLeftBracket:    return "UnmatchedLeftBracket";
        case ErrType::UnmatchedRightBracket:   return "UnmatchedRightBracket";
        case ErrType::IllegalNumber:           return "IllegalNumber";
        case ErrType::IllegalLabel:            return "IllegalLabel";
        default:                               return "<UNKNOWN>";
    }
}

// ============================================================================
// BasicType - 基本类型枚举（用于类型提示）
// ============================================================================

enum class BasicType : uint8_t {
    I8,      // i8
    I16,     // i16
    I32,     // i32
    I64,     // i64
    U8,      // u8
    U16,     // u16
    U32,     // u32
    U64,     // u64
    F32,     // f32
    F64,     // f64
    STR,     // str
    BOOL,    // bool
    UNIT,    // unit
    TUPLE,   // tuple
    LIST,    // list
    DICT,    // dict
};

// 基本类型名称表
constexpr std::array<std::string_view, 16> basic_type_names = {
    "i8", "i16", "i32", "i64",
    "u8", "u16", "u32", "u64",
    "f32", "f64",
    "str", "bool", "unit",
    "tuple", "list", "dict"
};

// 从字符串查找基本类型
constexpr bool is_basic_type(std::string_view name, BasicType& out) {
    for (size_t i = 0; i < basic_type_names.size(); ++i) {
        if (basic_type_names[i] == name) {
            out = static_cast<BasicType>(i);
            return true;
        }
    }
    return false;
}

// 获取基本类型名称
constexpr std::string_view get_basic_type_name(BasicType type) {
    auto idx = static_cast<size_t>(type);
    return idx < basic_type_names.size() ? basic_type_names[idx] : "";
}

} // namespace prim
