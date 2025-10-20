#include <functional>
#include <string>
#include <iostream>
#include <fstream>
#include <sstream>
#include <vector>
#include <fmt/format.h>

#include "token.hpp"
#include "lexer.hpp"
#include "debug.hpp"
#include "parser.hpp"

using fmt::println;
using namespace prim;

int main(int argc, char* argv[]) {
    bool lexer_only = false;
    const char* filename = nullptr;
    
    // 解析命令行参数
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--lexer-only") == 0) {
            lexer_only = true;
        } else if (strcmp(argv[i], "--help") == 0 || strcmp(argv[i], "-h") == 0) {
            println("Usage: {} [--lexer-only] <filename>", argv[0]);
            println("  --lexer-only    Only run lexical analysis");
            println("  --help, -h      Show this help message");
            return 0;
        } else if (filename == nullptr) {
            filename = argv[i];
        }
    }
    
    if (!filename) {
        println("Usage: {} [--lexer-only] <filename>", argv[0]);
        println("  --lexer-only    Only run lexical analysis");
        println("  --help, -h      Show this help message");
        filename = "/Users/wzq/Documents/Code/Project/jlu-cs/test.prim";
    }
    
    // 读取文件内容
    std::ifstream file(filename);
    if (!file) {
        println("Error: Cannot open file '{}'", filename);
        return 1;
    }
    
    std::stringstream buffer;
    buffer << file.rdbuf();
    std::string source = buffer.str();
    
    if (source.empty()) {
        println("Warning: File is empty");
        return 0;
    }
    
    println("╔════════════════════════════════════════════════════════════════╗");
    println("║                   Prim Language Compiler                       ║");
    println("╚════════════════════════════════════════════════════════════════╝");
    println("File: {}", filename);
    println("Size: {} bytes\n", source.size());
    
    // 第一步：词法分析 - 收集所有 tokens
    println("Phase 1: Lexical Analysis");
    println("─────────────────────────────────────────────────────────────────");
    
    Lexer lexer(source);
    std::vector<Token> tokens;
    
    while (true) {
        Token tok = lexer.next();
        
        // 遇到 END 或 ERROR 时停止，不添加到 tokens 中
        // (finalize() 会自己处理 END token)
        if (tok.type == TokenType::END || tok.type == TokenType::ERROR) {
            if (tok.type == TokenType::ERROR) {
                tokens.push_back(tok);  // 保留 ERROR token 用于调试
            }
            break;
        }
        
        tokens.push_back(tok);
    }
    
    println("✓ Collected {} tokens\n", tokens.size());
    
    // 打印 token 列表
    print_tokens(tokens);
    
    println("Lexical analysis completed successfully!\n");
    
    // 如果只需要词法分析，直接返回
    if (lexer_only) {
        return 0;
    }

    // 第二步：语法分析 - 解析 AST
    println("\nPhase 2: Syntax Analysis");
    println("─────────────────────────────────────────────────────────────────");
    
    // 创建新的 lexer 用于解析
    Lexer lexer2(source);
    Parser parser;
    
    // 使用 lambda 作为 token provider
    auto ast = parser.parse([&lexer2]() -> Token {
        return lexer2.next();
    });
    
    // 检查是否有解析错误
    if (parser.has_errors()) {
        println("\n❌ Parsing failed with {} error(s):\n", parser.get_errors().size());
        
        for (const auto& error : parser.get_errors()) {
            println("  Error at {}:{}:{}", 
                   filename,
                   error.location.line,
                   error.location.col);
            println("    {}", error.message);
            
            // 打印错误上下文（源码行）
            if (error.location.line > 0) {
                std::istringstream source_stream(source);
                std::string line;
                int current_line = 1;
                
                while (std::getline(source_stream, line) && current_line <= error.location.line) {
                    if (current_line == error.location.line) {
                        println("    | {}", line);
                        
                        // 打印指示符 (^)
                        // println 前面有4个空格，"| " 是2个字符，col是1-based
                        // 所以箭头位置是：2 (for "| ") + col - 1
                        if (error.location.col > 0) {
                            std::string indicator(2 + error.location.col, ' ');
                            indicator[2 + error.location.col - 1] = '^';
                            println("    {}", indicator);
                        }
                        break;
                    }
                    current_line++;
                }
            }
            println("");
        }
        
    }
    
    // 解析成功
    if (ast.has_value()) {
        println("✓ Parsing successful!");
        println("\nAST Root: Program");
        println("  Type: {}", static_cast<int>(ast->type));
        println("  Children: {}", ast->children.size());
        
        // 简单打印 AST 结构
        std::function<void(const ASTNode&, int)> print_ast;
        print_ast = [&print_ast](const ASTNode& node, int depth) {
            std::string indent(depth * 2, ' ');
            
            // 打印节点类型
            const char* type_names[] = {
                "Literal", "Identifier", "BinaryExpr", "UnaryExpr",
                "CallExpr", "IndexExpr", "FieldExpr",
                "TupleExpr", "ListExpr", "DictExpr", "DictPair",
                "BlockExpr", "ScopeExpr", "IfExpr", "LoopExpr",
                "LetStmt", "DelStmt", "BreakStmt", "ReturnStmt", "ExprStmt",
                "UnnamedPrim", "NamedPrim", "Param",
                "RefExpr", "LetTarget", "TypeHint",
                "StmtList", "ExprList", "LetTargetList", "IdentList", 
                "ParamList", "DecoratorList", "Program"
            };
            
            int type_idx = static_cast<int>(node.type);
            const char* type_name = (type_idx >= 0 && type_idx < 33) ? type_names[type_idx] : "Unknown";
            
            println("{}[{}]", indent, type_name);
            
            // 打印 token 信息（如果有）
            if (node.token) {
                println("{}  token: \"{}\"", indent, node.token->text);
            }
            
            // 打印特殊标志
            if (node.is_ref) println("{}  is_ref: true", indent);
            if (node.use_tail) println("{}  use_tail: true", indent);
            if (node.trailing_comma) println("{}  trailing_comma: true", indent);
            if (node.is_import) println("{}  is_import: true", indent);
            
            // 递归打印子节点（限制深度避免输出过长）
            if (depth < 5 && !node.children.empty()) {
                println("{}  children: {}", indent, node.children.size());
                for (const auto& child : node.children) {
                    print_ast(child, depth + 2);
                }
            } else if (!node.children.empty()) {
                println("{}  children: {} (depth limit reached)", indent, node.children.size());
            }
        };
        
        println("\nAST Structure (max depth 5):");
        println("─────────────────────────────────────────────────────────────────");
        print_ast(*ast, 0);
    } else {
        println("❌ Parsing failed: no AST generated");
        return 1;
    }
    
    println("\n╔════════════════════════════════════════════════════════════════╗");
    println("║                 Compilation Successful!                        ║");
    println("╚════════════════════════════════════════════════════════════════╝");
    
    return 0;
}
