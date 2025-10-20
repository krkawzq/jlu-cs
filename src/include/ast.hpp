#pragma once

#include <vector>
#include "token.hpp"

namespace prim {

// ===== 具体节点类型 =====
struct ASTNode {
    enum class NodeType {
        // ===== 字面量和标识符 =====
        Literal,        // 42, "str", true, false, null, () - token 存储原始值
        Identifier,     // x, foo - token 存储标识符名
        
        // ===== 运算符表达式 =====
        BinaryExpr,     // a + b, a && b, a == b, a = b - token 存储操作符
        UnaryExpr,      // !a, -a, +a - token 存储操作符
        
        // ===== 后缀表达式 =====
        CallExpr,       // func(args) - children: [callee, arg1, arg2, ...]
        IndexExpr,      // arr[idx] - children: [target, index]
        FieldExpr,      // obj.field - children: [target], token: field_name
        
        // ===== 容器 =====
        TupleExpr,      // (a, b, c) - children: [elem1, elem2, ...], 注意 (expr) 会直接解包
        ListExpr,       // [a, b, c] - children: [elem1, elem2, ...]
        DictExpr,       // {k: v, ...} - children: [pair1, pair2, ...], 空 {} 是空字典
        DictPair,       // k: v - children: [key, value]
        
        // ===== 块和控制流 =====
        // Block: 只用于 if 和 loop 的主体，语法强制要求
        // Scope: 普通作用域，用于函数体、匿名闭包等
        BlockExpr,      // if/loop 中的 {...} - children: [stmt1, stmt2, ...]
        ScopeExpr,      // 普通的 {...} 作用域 - children: [stmt1, stmt2, ...]
        
        IfExpr,         // if cond {...} else {...} - children: [cond, then_block, else_expr(opt)]
        LoopExpr,       // loop {...} or loop `label` {...} - children: [body], token: label(opt)
        
        // ===== 语句 =====
        LetStmt,        // let x = expr - children: [target_list, rhs(opt)]
        DelStmt,        // del x, y, z - children: [ident_list]
        BreakStmt,      // break or break `label` or break expr - children: [value(opt)], token: label(opt)
        ReturnStmt,     // return or return expr - children: [value(opt)]
        ExprStmt,       // expr; - children: [expr]
        
        // ===== Prim（函数） =====
        // 匿名 Prim: @{...} 或 @dec1 @dec2 @{...}
        // 命名 Prim: $name(params) {...} 或 @dec $name(params) {...}
        UnnamedPrim,    // @{...} - children: [decorator_list(opt), scope]
        NamedPrim,      // $name(params) {...} - children: [decorator_list(opt), param_list, return_type(opt), impl]
                        // token: name, impl 是 scope
        Param,          // x 或 &x - children: [type_hint(opt)], token: name
        
        // ===== 引用 =====
        RefExpr,        // &expr - children: [target]
                        // 注意：Ref 不能隐式转换为 Expr，是完全独立的类型
        
        // ===== Let Target =====
        LetTarget,      // let 的目标：x 或 &x - children: [type_hint(opt)], token: name
        
        // ===== 辅助节点 =====
        TypeHint,       // i32 | str | null - children: [ident1, ident2, ...] 变长数组
        
        // ===== 列表节点 =====
        StmtList,       // 语句列表 - children: [stmt1, stmt2, ...]
        ExprList,       // 表达式/引用列表（用于tuple、list、call参数等）- children: [expr_or_ref1, ...]
        LetTargetList,  // let 目标列表 - children: [target1, target2, ...]
        IdentList,      // 标识符列表（用于del）- children: [ident1, ident2, ...]
        ParamList,      // 参数列表 - children: [param1, param2, ...]
        DecoratorList,  // 装饰器列表 - children: [ident1, ident2, ...] 存储装饰器名
        
        // ===== 根节点 =====
        Program,        // 整个程序 - children: [stmt_list]
    };
    
    NodeType type;
    std::vector<ASTNode> children;  // 使用 vector 而不是 list，更高效
    const Token* token = nullptr;   // 存储关键 token（操作符、标识符、字面量等）
    
    // ===== 辅助字段 =====
    // 注意：这些字段根据节点类型使用不同的含义
    bool is_ref = false;            // 用于 Param 和 LetTarget，表示是否是引用(&)
    bool use_tail = false;          // 用于 BlockExpr 和 ScopeExpr，表示是否使用最后的表达式作为返回值
    bool trailing_comma = false;    // 用于 TupleExpr，单元素 tuple 必须有尾随逗号: (x,)
    bool is_import = false;         // 用于 LetStmt，区分导入外部变量 (let x;) 和定义新变量 (let x = expr;)
    
    // 构造函数
    ASTNode() : type(NodeType::Program) {}  // 默认构造函数
    ASTNode(NodeType t) : type(t) {}
    ASTNode(NodeType t, const Token* tok) : type(t), token(tok) {}
};

} // namespace prim
