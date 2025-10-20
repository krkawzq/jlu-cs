#pragma once

#include "ast.hpp"
#include "token.hpp"
#include "parse_error.hpp"
#include <functional>
#include <vector>
#include <memory>
#include <optional>

namespace prim {

// 前向声明 Bison 生成的 parser 类
namespace detail {
    class BisonParser;
}

// ============================================================================
// Parser - 语法分析器
// ============================================================================

class Parser {
public:
    using TokenProvider = std::function<Token()>;
    
    // 构造和析构
    Parser();
    ~Parser();
    
    // 禁止拷贝，允许移动
    Parser(const Parser&) = delete;
    Parser& operator=(const Parser&) = delete;
    Parser(Parser&&) noexcept;
    Parser& operator=(Parser&&) noexcept;
    
    // ===== 主要接口 =====
    
    /**
     * 解析 token 流，生成 AST
     * @param token_provider 提供 token 的闭包函数
     * @return 如果解析成功返回 AST 根节点，否则返回 nullopt
     * 
     * 注意：
     * - parser 会一直调用 token_provider 直到遇到 END token
     * - 遇到 END 后会停止解析，后续的 token 将被忽略
     * - 解析完成后调用 get_errors() 获取错误列表
     */
    std::optional<ASTNode> parse(TokenProvider token_provider);
    
    // ===== 错误相关 =====
    
    /**
     * 获取所有解析错误
     */
    const std::vector<ParseError>& get_errors() const { return errors_; }
    
    /**
     * 检查是否有错误
     */
    bool has_errors() const { return !errors_.empty(); }
    
    /**
     * 清空错误列表
     */
    void clear_errors() { errors_.clear(); }
    
    // ===== 状态管理 =====
    
    /**
     * 重置 parser 到初始状态
     * - 清空错误列表
     * - 清空 AST 结果
     * - 重新创建内部 parser 对象
     */
    void reset();
    
private:
    std::unique_ptr<detail::BisonParser> bison_parser_;
    std::vector<ParseError> errors_;
    std::optional<ASTNode> result_;
    TokenProvider token_provider_;  // 保存当前的 token provider
    std::vector<Token> token_storage_;  // 存储 Token 对象以保持生命周期
    
    // Bison parser 需要访问私有成员
    friend class detail::BisonParser;
    
    // ===== 内部辅助方法（由 Bison/yylex 调用，不应被外部使用）=====
public:  // 这些方法技术上是public的，但仅供内部使用
    /**
     * 添加解析错误（由 Bison parser 调用）
     * @internal 仅供 Bison 内部使用
     */
    void add_error(ParseError error);
    
    /**
     * 设置解析结果（由 Bison parser 调用）
     * @internal 仅供 Bison 内部使用
     */
    void set_result(ASTNode node);
    
    /**
     * 获取下一个 token（由 yylex 调用）
     * @internal 仅供 yylex 内部使用
     */
    Token next_token();
    
    /**
     * 存储 token 并返回指针（由 yylex 调用）
     * @internal 仅供 yylex 内部使用，用于保持 token 生命周期
     */
    const Token* store_token(Token tok);
};

} // namespace prim

