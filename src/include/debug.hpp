// debug.hpp - Prim 调试输出工具
#pragma once

#include <string>
#include <vector>
#include <fmt/format.h>
#include <magic_enum.hpp>

#include "token.hpp"

namespace prim {

using fmt::print, fmt::println;

// ============================================================================
// Token 列表打印
// ============================================================================

inline void print_tokens(const std::vector<Token>& tokens) {
    for (size_t i = 0; i < tokens.size(); ++i) {
        const auto& token = tokens[i];
        
        print("[{:3}] ", i);
        print("{:<15} ", magic_enum::enum_name(token.type));
        
        if (!token.text.empty()) {
            print("'{}'", token.text);
        }
        
        print(" @ {}:{}", token.begin.line, token.begin.col);
        
        if (token.err != ErrType::None) {
            print(" [ERROR: {}]", magic_enum::enum_name(token.err));
        }
        
        println("");
    }
    println("");
}

} // namespace prim