#pragma once

#include "token.hpp"
#include <string>
#include <fmt/format.h>

namespace prim {

// ============================================================================
// ParseErrorType - 解析错误类型
// ============================================================================

enum class ParseErrorType {
    UnexpectedToken,      // 意外的 token
    MissingToken,         // 缺少 token（如缺少分号、括号等）
    EmptyBlock,           // 空 block（if/loop/prim 后不能为空）
    InvalidSyntax,        // 一般语法错误
    UnexpectedEOF,        // 意外的文件结束
    DuplicateDecorator,   // 重复的装饰器
    InvalidLetTarget,     // 无效的 let 目标
    InvalidDelTarget,     // del 只能删除标识符
};

// ============================================================================
// ParseError - 解析错误信息
// ============================================================================

struct ParseError {
    ParseErrorType type;
    Location location;
    std::string message;
    std::string context;  // 错误上下文（如出错的那一行代码）
    
    // 构造函数
    ParseError(ParseErrorType t, Location loc, std::string msg, std::string ctx = "")
        : type(t), location(loc), message(std::move(msg)), context(std::move(ctx)) {}
    
    // 格式化为可读的错误信息
    std::string format() const {
        std::string type_str;
        switch (type) {
            case ParseErrorType::UnexpectedToken:
                type_str = "Unexpected token";
                break;
            case ParseErrorType::MissingToken:
                type_str = "Missing token";
                break;
            case ParseErrorType::EmptyBlock:
                type_str = "Empty block";
                break;
            case ParseErrorType::InvalidSyntax:
                type_str = "Syntax error";
                break;
            case ParseErrorType::UnexpectedEOF:
                type_str = "Unexpected end of file";
                break;
            case ParseErrorType::DuplicateDecorator:
                type_str = "Duplicate decorator";
                break;
            case ParseErrorType::InvalidLetTarget:
                type_str = "Invalid let target";
                break;
            case ParseErrorType::InvalidDelTarget:
                type_str = "Invalid del target";
                break;
        }
        
        std::string result = fmt::format("{}:{}:{}: {}: {}",
            "<input>",  // 文件名（可以后续添加）
            location.line,
            location.col,
            type_str,
            message
        );
        
        if (!context.empty()) {
            result += fmt::format("\n  {}", context);
        }
        
        return result;
    }
    
    // 获取错误类型的字符串表示
    static const char* type_to_string(ParseErrorType type) {
        switch (type) {
            case ParseErrorType::UnexpectedToken: return "UnexpectedToken";
            case ParseErrorType::MissingToken: return "MissingToken";
            case ParseErrorType::EmptyBlock: return "EmptyBlock";
            case ParseErrorType::InvalidSyntax: return "InvalidSyntax";
            case ParseErrorType::UnexpectedEOF: return "UnexpectedEOF";
            case ParseErrorType::DuplicateDecorator: return "DuplicateDecorator";
            case ParseErrorType::InvalidLetTarget: return "InvalidLetTarget";
            case ParseErrorType::InvalidDelTarget: return "InvalidDelTarget";
            default: return "Unknown";
        }
    }
};

} // namespace prim

