#include "parser.hpp"
#include "parser.tab.hpp"  // Bison 生成的头文件
#include <stdexcept>

namespace prim {

// ============================================================================
// Parser 类实现
// ============================================================================

Parser::Parser() 
    : bison_parser_(std::make_unique<detail::BisonParser>(*this)) 
{}

Parser::~Parser() = default;

Parser::Parser(Parser&&) noexcept = default;
Parser& Parser::operator=(Parser&&) noexcept = default;

std::optional<ASTNode> Parser::parse(TokenProvider token_provider) {
    // 重置状态
    reset();
    
    // 保存 token provider
    token_provider_ = std::move(token_provider);
    
    // 调用 Bison parser
    int result = bison_parser_->parse();
    
    // 如果解析成功且没有错误，返回结果
    if (result == 0 && !has_errors()) {
        return std::move(result_);
    }
    
    return std::nullopt;
}

void Parser::reset() {
    errors_.clear();
    result_.reset();
    token_provider_ = nullptr;
    token_storage_.clear();
    bison_parser_ = std::make_unique<detail::BisonParser>(*this);
}

void Parser::add_error(ParseError error) {
    errors_.push_back(std::move(error));
}

void Parser::set_result(ASTNode node) {
    result_ = std::move(node);
}

Token Parser::next_token() {
    if (token_provider_) {
        return token_provider_();
    }
    // 如果没有 provider，返回 END token
    Token tok;
    tok.type = TokenType::END;
    tok.begin = Location{0, 0, 0};
    tok.end = Location{0, 0, 0};
    return tok;
}

const Token* Parser::store_token(Token tok) {
    token_storage_.push_back(std::move(tok));
    return &token_storage_.back();
}

// ============================================================================
// yylex 实现 - 将 Token 转换为 Bison symbol
// ============================================================================

namespace detail {

BisonParser::symbol_type yylex(Parser& parser) {
    Token tok = parser.next_token();
    
    // 存储 Token 以保持生命周期，并获取指针
    const Token* tok_ptr = parser.store_token(std::move(tok));
    
    // 创建 location 信息
    BisonParser::location_type loc;
    loc.begin.line = tok_ptr->begin.line;
    loc.begin.column = tok_ptr->begin.col;
    loc.end.line = tok_ptr->end.line;
    loc.end.column = tok_ptr->end.col;
    
    // 根据 token 类型返回相应的 symbol
    switch (tok_ptr->type) {
        case TokenType::END:
            return BisonParser::make_END(loc);
        
        // 关键字
        case TokenType::KW_LET:
            return BisonParser::make_KW_LET(loc);
        case TokenType::KW_DEL:
            return BisonParser::make_KW_DEL(loc);
        case TokenType::KW_IF:
            return BisonParser::make_KW_IF(loc);
        case TokenType::KW_ELSE:
            return BisonParser::make_KW_ELSE(loc);
        case TokenType::KW_LOOP:
            return BisonParser::make_KW_LOOP(loc);
        case TokenType::KW_BREAK:
            return BisonParser::make_KW_BREAK(loc);
        case TokenType::KW_RETURN:
            return BisonParser::make_KW_RETURN(loc);
        case TokenType::KW_TRUE:
            return BisonParser::make_KW_TRUE(tok_ptr, loc);
        case TokenType::KW_FALSE:
            return BisonParser::make_KW_FALSE(tok_ptr, loc);
        case TokenType::KW_NULL:
            return BisonParser::make_KW_NULL(tok_ptr, loc);
        
        // 标识符和字面量（需要传递 Token 指针）
        case TokenType::IDENT:
            return BisonParser::make_IDENT(tok_ptr, loc);
        case TokenType::INT_DEC:
            return BisonParser::make_INT_DEC(tok_ptr, loc);
        case TokenType::INT_HEX:
            return BisonParser::make_INT_HEX(tok_ptr, loc);
        case TokenType::INT_OCT:
            return BisonParser::make_INT_OCT(tok_ptr, loc);
        case TokenType::INT_BIN:
            return BisonParser::make_INT_BIN(tok_ptr, loc);
        case TokenType::FLOAT_DEC:
            return BisonParser::make_FLOAT_DEC(tok_ptr, loc);
        case TokenType::STRING:
            return BisonParser::make_STRING(tok_ptr, loc);
        case TokenType::LABEL:
            return BisonParser::make_LABEL(tok_ptr, loc);
        
        // 运算符
        case TokenType::PLUS:
            return BisonParser::make_PLUS(tok_ptr, loc);
        case TokenType::MINUS:
            return BisonParser::make_MINUS(tok_ptr, loc);
        case TokenType::STAR:
            return BisonParser::make_STAR(tok_ptr, loc);
        case TokenType::SLASH:
            return BisonParser::make_SLASH(tok_ptr, loc);
        case TokenType::PERCENT:
            return BisonParser::make_PERCENT(tok_ptr, loc);
        case TokenType::EQ:
            return BisonParser::make_EQ(tok_ptr, loc);
        case TokenType::EQEQ:
            return BisonParser::make_EQEQ(tok_ptr, loc);
        case TokenType::NEQ:
            return BisonParser::make_NEQ(tok_ptr, loc);
        case TokenType::LT:
            return BisonParser::make_LT(tok_ptr, loc);
        case TokenType::GT:
            return BisonParser::make_GT(tok_ptr, loc);
        case TokenType::LE:
            return BisonParser::make_LE(tok_ptr, loc);
        case TokenType::GE:
            return BisonParser::make_GE(tok_ptr, loc);
        case TokenType::ANDAND:
            return BisonParser::make_ANDAND(tok_ptr, loc);
        case TokenType::OROR:
            return BisonParser::make_OROR(tok_ptr, loc);
        case TokenType::BANG:
            return BisonParser::make_BANG(tok_ptr, loc);
        case TokenType::AMP:
            return BisonParser::make_AMP(tok_ptr, loc);
        
        // 分隔符
        case TokenType::LPAREN:
            return BisonParser::make_LPAREN(loc);
        case TokenType::RPAREN:
            return BisonParser::make_RPAREN(loc);
        case TokenType::LBRACE:
            return BisonParser::make_LBRACE(loc);
        case TokenType::RBRACE:
            return BisonParser::make_RBRACE(loc);
        case TokenType::LBRACK:
            return BisonParser::make_LBRACK(loc);
        case TokenType::RBRACK:
            return BisonParser::make_RBRACK(loc);
        case TokenType::SEMI:
            return BisonParser::make_SEMI(loc);
        case TokenType::COMMA:
            return BisonParser::make_COMMA(loc);
        case TokenType::COLON:
            return BisonParser::make_COLON(loc);
        case TokenType::AT:
            return BisonParser::make_AT(loc);
        case TokenType::DOLLAR:
            return BisonParser::make_DOLLAR(loc);
        case TokenType::DOT:
            return BisonParser::make_DOT(loc);
        case TokenType::PIPE:
            return BisonParser::make_PIPE(loc);
        
        // 错误 token
        case TokenType::ERROR:
            parser.add_error(ParseError{
                ParseErrorType::UnexpectedToken,
                tok_ptr->begin,
                "Lexical error"
            });
            return BisonParser::make_END(loc);
        
        default:
            parser.add_error(ParseError{
                ParseErrorType::UnexpectedToken,
                tok_ptr->begin,
                "Unknown token type"
            });
            return BisonParser::make_END(loc);
    }
}

} // namespace detail

} // namespace prim

