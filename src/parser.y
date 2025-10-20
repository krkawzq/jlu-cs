%{
// parser.y - Prim 语言 Bison 语法文件
#include "parse_error.hpp"
#include <utility>
%}

/* ============================================================================
 * Bison 配置
 * ============================================================================
 */

%skeleton "lalr1.cc"
%require "3.2"
%language "c++"

/* API 配置 */
%define api.namespace {prim::detail}
%define api.parser.class {BisonParser}
%define api.token.constructor
%define api.value.type variant
%locations
%define parse.error verbose
%define parse.trace

/* ============================================================================
 * 代码块
 * ============================================================================
 */

%code requires {
    #include "ast.hpp"
    #include "token.hpp"
    #include <optional>
    
    namespace prim {
        class Parser;
    }
}

%param { prim::Parser& parser }

%code {
    #include "parser.hpp"
    
    namespace prim::detail {
        // yylex 函数声明
        BisonParser::symbol_type yylex(prim::Parser& parser);
    }
    
    // AST 构建辅助函数
    namespace {
        using namespace prim;
        
        // ===== 列表节点 =====
        
        ASTNode create_stmt_list() {
            ASTNode node(ASTNode::NodeType::StmtList);
            return node;
        }
        
        void stmt_list_add(ASTNode& list, ASTNode stmt) {
            list.children.push_back(std::move(stmt));
        }
        
        ASTNode create_expr_list() {
            ASTNode node(ASTNode::NodeType::ExprList);
            return node;
        }
        
        void expr_list_add(ASTNode& list, ASTNode expr) {
            list.children.push_back(std::move(expr));
        }
        
        ASTNode create_let_target_list() {
            ASTNode node(ASTNode::NodeType::LetTargetList);
            return node;
        }
        
        void let_target_list_add(ASTNode& list, ASTNode target) {
            list.children.push_back(std::move(target));
        }
        
        ASTNode create_ident_list() {
            ASTNode node(ASTNode::NodeType::IdentList);
            return node;
        }
        
        void ident_list_add(ASTNode& list, const Token* ident) {
            ASTNode node(ASTNode::NodeType::Identifier, ident);
            list.children.push_back(std::move(node));
        }
        
        ASTNode create_param_list() {
            ASTNode node(ASTNode::NodeType::ParamList);
            return node;
        }
        
        void param_list_add(ASTNode& list, ASTNode param) {
            list.children.push_back(std::move(param));
        }
        
        ASTNode create_decorator_list() {
            ASTNode node(ASTNode::NodeType::DecoratorList);
            return node;
        }
        
        void decorator_list_add(ASTNode& list, const Token* decorator) {
            ASTNode node(ASTNode::NodeType::Identifier, decorator);
            list.children.push_back(std::move(node));
        }
        
        // ===== 基本节点 =====
        
        ASTNode create_program(ASTNode stmt_list) {
            ASTNode node(ASTNode::NodeType::Program);
            node.children.push_back(std::move(stmt_list));
            return node;
        }
        
        ASTNode create_literal(const Token* tok) {
            return ASTNode(ASTNode::NodeType::Literal, tok);
        }
        
        ASTNode create_identifier(const Token* tok) {
            return ASTNode(ASTNode::NodeType::Identifier, tok);
        }
        
        // ===== 表达式 =====
        
        ASTNode create_binary_expr(const Token* op, ASTNode left, ASTNode right) {
            ASTNode node(ASTNode::NodeType::BinaryExpr, op);
            node.children.push_back(std::move(left));
            node.children.push_back(std::move(right));
            return node;
        }
        
        ASTNode create_unary_expr(const Token* op, ASTNode operand) {
            ASTNode node(ASTNode::NodeType::UnaryExpr, op);
            node.children.push_back(std::move(operand));
            return node;
        }
        
        ASTNode create_call_expr(ASTNode callee, ASTNode args) {
            ASTNode node(ASTNode::NodeType::CallExpr);
            node.children.push_back(std::move(callee));
            // 将 args 的所有 children 添加到 call node
            for (auto& arg : args.children) {
                node.children.push_back(std::move(arg));
            }
            return node;
        }
        
        ASTNode create_index_expr(ASTNode target, ASTNode index) {
            ASTNode node(ASTNode::NodeType::IndexExpr);
            node.children.push_back(std::move(target));
            node.children.push_back(std::move(index));
            return node;
        }
        
        ASTNode create_field_expr(ASTNode target, const Token* field) {
            ASTNode node(ASTNode::NodeType::FieldExpr, field);
            node.children.push_back(std::move(target));
            return node;
        }
        
        ASTNode create_ref_expr(ASTNode target) {
            ASTNode node(ASTNode::NodeType::RefExpr);
            node.children.push_back(std::move(target));
            return node;
        }
        
        // ===== 容器 =====
        
        ASTNode create_tuple_expr(ASTNode elements, bool trailing_comma) {
            ASTNode node(ASTNode::NodeType::TupleExpr);
            node.trailing_comma = trailing_comma;
            for (auto& elem : elements.children) {
                node.children.push_back(std::move(elem));
            }
            return node;
        }
        
        ASTNode create_list_expr(ASTNode elements) {
            ASTNode node(ASTNode::NodeType::ListExpr);
            for (auto& elem : elements.children) {
                node.children.push_back(std::move(elem));
            }
            return node;
        }
        
        ASTNode create_dict_expr(ASTNode pairs) {
            ASTNode node(ASTNode::NodeType::DictExpr);
            for (auto& pair : pairs.children) {
                node.children.push_back(std::move(pair));
            }
            return node;
        }
        
        ASTNode create_dict_pair(ASTNode key, ASTNode value) {
            ASTNode node(ASTNode::NodeType::DictPair);
            node.children.push_back(std::move(key));
            node.children.push_back(std::move(value));
            return node;
        }
        
        // ===== Block 和 Scope =====
        
        ASTNode create_block_expr(ASTNode stmts, bool use_tail) {
            ASTNode node(ASTNode::NodeType::BlockExpr);
            node.use_tail = use_tail;
            for (auto& stmt : stmts.children) {
                node.children.push_back(std::move(stmt));
            }
            return node;
        }
        
        ASTNode create_scope_expr(ASTNode stmts, bool use_tail) {
            ASTNode node(ASTNode::NodeType::ScopeExpr);
            node.use_tail = use_tail;
            for (auto& stmt : stmts.children) {
                node.children.push_back(std::move(stmt));
            }
            return node;
        }
        
        // ===== 控制流 =====
        
        ASTNode create_if_expr(ASTNode cond, ASTNode then_block, std::optional<ASTNode> else_expr) {
            ASTNode node(ASTNode::NodeType::IfExpr);
            node.children.push_back(std::move(cond));
            node.children.push_back(std::move(then_block));
            if (else_expr.has_value()) {
                node.children.push_back(std::move(*else_expr));
            }
            return node;
        }
        
        ASTNode create_loop_expr(const Token* label, ASTNode body) {
            ASTNode node(ASTNode::NodeType::LoopExpr, label);
            node.children.push_back(std::move(body));
            return node;
        }
        
        // ===== 语句 =====
        
        ASTNode create_let_stmt(ASTNode targets, std::optional<ASTNode> rhs) {
            ASTNode node(ASTNode::NodeType::LetStmt);
            node.is_import = !rhs.has_value();
            node.children.push_back(std::move(targets));
            if (rhs.has_value()) {
                node.children.push_back(std::move(*rhs));
            }
            return node;
        }
        
        ASTNode create_let_target(const Token* name, std::optional<ASTNode> type_hint, bool is_ref) {
            ASTNode node(ASTNode::NodeType::LetTarget, name);
            node.is_ref = is_ref;
            if (type_hint.has_value()) {
                node.children.push_back(std::move(*type_hint));
            }
            return node;
        }
        
        ASTNode create_del_stmt(ASTNode idents) {
            ASTNode node(ASTNode::NodeType::DelStmt);
            node.children.push_back(std::move(idents));
            return node;
        }
        
        ASTNode create_break_stmt(const Token* label, std::optional<ASTNode> value) {
            ASTNode node(ASTNode::NodeType::BreakStmt, label);
            if (value.has_value()) {
                node.children.push_back(std::move(*value));
            }
            return node;
        }
        
        ASTNode create_return_stmt(std::optional<ASTNode> value) {
            ASTNode node(ASTNode::NodeType::ReturnStmt);
            if (value.has_value()) {
                node.children.push_back(std::move(*value));
            }
            return node;
        }
        
        ASTNode create_expr_stmt(ASTNode expr) {
            ASTNode node(ASTNode::NodeType::ExprStmt);
            node.children.push_back(std::move(expr));
            return node;
        }
        
        // ===== Prim =====
        
        ASTNode create_unnamed_prim(ASTNode decorators, ASTNode scope) {
            ASTNode node(ASTNode::NodeType::UnnamedPrim);
            node.children.push_back(std::move(decorators));
            node.children.push_back(std::move(scope));
            return node;
        }
        
        ASTNode create_named_prim(ASTNode decorators, const Token* name, ASTNode params, 
                                  std::optional<ASTNode> return_type, ASTNode impl) {
            ASTNode node(ASTNode::NodeType::NamedPrim, name);
            node.children.push_back(std::move(decorators));
            node.children.push_back(std::move(params));
            if (return_type.has_value()) {
                node.children.push_back(std::move(*return_type));
            }
            node.children.push_back(std::move(impl));
            return node;
        }
        
        ASTNode create_param(const Token* name, std::optional<ASTNode> type_hint, bool is_ref) {
            ASTNode node(ASTNode::NodeType::Param, name);
            node.is_ref = is_ref;
            if (type_hint.has_value()) {
                node.children.push_back(std::move(*type_hint));
            }
            return node;
        }
        
        // ===== 类型提示 =====
        
        ASTNode create_type_hint() {
            return ASTNode(ASTNode::NodeType::TypeHint);
        }
        
        void type_hint_add(ASTNode& hint, const Token* ident) {
            ASTNode node(ASTNode::NodeType::Identifier, ident);
            hint.children.push_back(std::move(node));
        }
    }
}

/* ============================================================================
 * Token 和类型定义
 * ============================================================================
 */

%token END 0 "end of file"

/* 关键字 */
%token KW_LET "let"
%token KW_DEL "del"
%token KW_IF "if"
%token KW_ELSE "else"
%token KW_LOOP "loop"
%token KW_BREAK "break"
%token KW_RETURN "return"
%token <const Token*> KW_TRUE "true"
%token <const Token*> KW_FALSE "false"
%token <const Token*> KW_NULL "null"

/* 标识符和字面量 - 携带 Token 指针 */
%token <const Token*> IDENT "identifier"
%token <const Token*> INT_DEC "int_dec"
%token <const Token*> INT_HEX "int_hex"
%token <const Token*> INT_OCT "int_oct"
%token <const Token*> INT_BIN "int_bin"
%token <const Token*> FLOAT_DEC "float"
%token <const Token*> STRING "string"
%token <const Token*> LABEL "label"

/* 运算符 */
%token <const Token*> PLUS "+"
%token <const Token*> MINUS "-"
%token <const Token*> STAR "*"
%token <const Token*> SLASH "/"
%token <const Token*> PERCENT "%"
%token <const Token*> EQ "="
%token <const Token*> EQEQ "=="
%token <const Token*> NEQ "!="
%token <const Token*> LT "<"
%token <const Token*> GT ">"
%token <const Token*> LE "<="
%token <const Token*> GE ">="
%token <const Token*> ANDAND "&&"
%token <const Token*> OROR "||"
%token <const Token*> BANG "!"
%token <const Token*> AMP "&"

/* 分隔符 */
%token LPAREN "("
%token RPAREN ")"
%token LBRACE "{"
%token RBRACE "}"
%token LBRACK "["
%token RBRACK "]"
%token SEMI ";"
%token COMMA ","
%token COLON ":"
%token AT "@"
%token DOLLAR "$"
%token DOT "."
%token PIPE "|"

/* 非终结符类型 */
%type <ASTNode> program
%type <ASTNode> stmt_list stmt_list_opt
%type <ASTNode> stmt
%type <ASTNode> let_stmt del_stmt break_stmt return_stmt expr_stmt

%type <ASTNode> expr
%type <ASTNode> assignment_expr
%type <ASTNode> logical_or_expr logical_and_expr
%type <ASTNode> equality_expr relational_expr
%type <ASTNode> additive_expr multiplicative_expr
%type <ASTNode> unary_expr postfix_expr primary_expr

%type <ASTNode> scope_expr block_expr
%type <ASTNode> if_expr loop_expr
%type <ASTNode> tuple_expr list_expr dict_expr
%type <ASTNode> unnamed_prim named_prim
%type <ASTNode> ref_expr

%type <ASTNode> dict_pair_list dict_pair
%type <ASTNode> expr_list expr_list_opt
%type <ASTNode> let_target_list let_target
%type <ASTNode> ident_list
%type <ASTNode> param_list param_list_opt param
%type <ASTNode> decorators
%type <ASTNode> type_hint
%type <std::optional<ASTNode>> type_hint_opt

%type <std::optional<ASTNode>> if_else_chain
%type <const Token*> label_opt

/* ============================================================================
 * 运算符优先级
 * ============================================================================
 * 注意: 我们使用显式的语法规则层次来定义优先级，不需要 %left/%right 声明
 * 优先级通过以下规则层次体现 (从低到高):
 *   assignment_expr → logical_or_expr → logical_and_expr → equality_expr
 *   → relational_expr → additive_expr → multiplicative_expr → unary_expr
 *   → postfix_expr → primary_expr
 */

/* 开始符号 */
%start program

%%

/* ============================================================================
 * 语法规则
 * ============================================================================
 */

/* ────────────────────────────────────────────────────────────────────────────
 * 程序结构
 * ────────────────────────────────────────────────────────────────────────────
 */

program
    : stmt_list_opt END {
        $$ = create_program($1);
        parser.set_result($$);
    }
    ;

stmt_list_opt
    : %empty {
        $$ = create_stmt_list();
    }
    | stmt_list {
        $$ = $1;
    }
    ;

stmt_list
    : stmt {
        $$ = create_stmt_list();
        stmt_list_add($$, $1);
    }
    | stmt_list ";" stmt {
        stmt_list_add($1, $3);
        $$ = $1;
    }
    | stmt_list ";" {
        /* 尾随分号，不添加任何东西 */
        $$ = $1;
    }
    ;

stmt
    : let_stmt { $$ = $1; }
    | del_stmt { $$ = $1; }
    | break_stmt { $$ = $1; }
    | return_stmt { $$ = $1; }
    | expr_stmt { $$ = $1; }
    ;

/* ────────────────────────────────────────────────────────────────────────────
 * 语句
 * ────────────────────────────────────────────────────────────────────────────
 */

/* Let 语句 */
let_stmt
    : "let" let_target_list {
        /* 导入外部变量 */
        $$ = create_let_stmt($2, std::nullopt);
    }
    | "let" let_target_list "=" expr {
        /* 定义新变量或解包 */
        $$ = create_let_stmt($2, $4);
    }
    | "let" let_target_list "=" ref_expr {
        /* let x = &y; */
        $$ = create_let_stmt($2, $4);
    }
    ;

let_target_list
    : let_target {
        $$ = create_let_target_list();
        let_target_list_add($$, $1);
    }
    | let_target_list "," let_target {
        let_target_list_add($1, $3);
        $$ = $1;
    }
    ;

let_target
    : "identifier" type_hint_opt {
        $$ = create_let_target($1, $2, false);
    }
    | "&" "identifier" type_hint_opt {
        $$ = create_let_target($2, $3, true);
    }
    ;

/* Del 语句 */
del_stmt
    : "del" ident_list {
        $$ = create_del_stmt($2);
    }
    ;

ident_list
    : "identifier" {
        $$ = create_ident_list();
        ident_list_add($$, $1);
    }
    | ident_list "," "identifier" {
        ident_list_add($1, $3);
        $$ = $1;
    }
    ;

/* Break 语句 */
break_stmt
    : "break" {
        $$ = create_break_stmt(nullptr, std::nullopt);
    }
    | "break" "label" {
        $$ = create_break_stmt($2, std::nullopt);
    }
    | "break" expr {
        $$ = create_break_stmt(nullptr, $2);
    }
    | "break" ref_expr {
        $$ = create_break_stmt(nullptr, $2);
    }
    | "break" "label" expr {
        $$ = create_break_stmt($2, $3);
    }
    | "break" "label" ref_expr {
        $$ = create_break_stmt($2, $3);
    }
    ;

/* Return 语句 */
return_stmt
    : "return" {
        $$ = create_return_stmt(std::nullopt);
    }
    | "return" expr {
        $$ = create_return_stmt($2);
    }
    | "return" ref_expr {
        $$ = create_return_stmt($2);
    }
    ;

/* 表达式语句 */
expr_stmt
    : expr {
        $$ = create_expr_stmt($1);
    }
    ;

/* ────────────────────────────────────────────────────────────────────────────
 * 表达式（按优先级从低到高）
 * ────────────────────────────────────────────────────────────────────────────
 */

expr
    : assignment_expr { $$ = $1; }
    ;

/* 赋值（右结合） */
assignment_expr
    : logical_or_expr { $$ = $1; }
    | logical_or_expr "=" assignment_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    ;

/* 逻辑或 */
logical_or_expr
    : logical_and_expr { $$ = $1; }
    | logical_or_expr "||" logical_and_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    ;

/* 逻辑与 */
logical_and_expr
    : equality_expr { $$ = $1; }
    | logical_and_expr "&&" equality_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    ;

/* 相等性 */
equality_expr
    : relational_expr { $$ = $1; }
    | equality_expr "==" relational_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    | equality_expr "!=" relational_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    ;

/* 关系比较 */
relational_expr
    : additive_expr { $$ = $1; }
    | relational_expr "<" additive_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    | relational_expr ">" additive_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    | relational_expr "<=" additive_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    | relational_expr ">=" additive_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    ;

/* 加减 */
additive_expr
    : multiplicative_expr { $$ = $1; }
    | additive_expr "+" multiplicative_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    | additive_expr "-" multiplicative_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    ;

/* 乘除模 */
multiplicative_expr
    : unary_expr { $$ = $1; }
    | multiplicative_expr "*" unary_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    | multiplicative_expr "/" unary_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    | multiplicative_expr "%" unary_expr {
        $$ = create_binary_expr($2, $1, $3);
    }
    ;

/* 一元运算符 */
unary_expr
    : postfix_expr { $$ = $1; }
    | "!" unary_expr {
        $$ = create_unary_expr($1, $2);
    }
    | "+" unary_expr {
        /* 一元加号: +expr (语法层次已确保与二元+不冲突) */
        $$ = create_unary_expr($1, $2);
    }
    | "-" unary_expr {
        /* 一元减号: -expr (语法层次已确保与二元-不冲突) */
        $$ = create_unary_expr($1, $2);
    }
    ;

/* 后缀运算符 */
postfix_expr
    : primary_expr { $$ = $1; }
    | postfix_expr "(" expr_list_opt ")" {
        $$ = create_call_expr($1, $3);
    }
    | postfix_expr "[" expr "]" {
        $$ = create_index_expr($1, $3);
    }
    | postfix_expr "." "identifier" {
        $$ = create_field_expr($1, $3);
    }
    ;

/* 基本表达式 */
primary_expr
    /* 字面量 */
    : "int_dec" { $$ = create_literal($1); }
    | "int_hex" { $$ = create_literal($1); }
    | "int_oct" { $$ = create_literal($1); }
    | "int_bin" { $$ = create_literal($1); }
    | "float" { $$ = create_literal($1); }
    | "string" { $$ = create_literal($1); }
    | "true" { $$ = create_literal($1); }
    | "false" { $$ = create_literal($1); }
    | "null" { $$ = create_literal($1); }
    
    /* 标识符 */
    | "identifier" { $$ = create_identifier($1); }
    
    /* () = null */
    | "(" ")" {
        $$ = create_literal(nullptr);  // null literal
    }
    
    /* (expr) = 括号消除 */
    | "(" expr ")" {
        $$ = $2;  // 直接返回表达式
    }
    
    /* 复杂表达式 */
    | tuple_expr { $$ = $1; }
    | list_expr { $$ = $1; }
    | dict_expr { $$ = $1; }
    | scope_expr { $$ = $1; }
    | if_expr { $$ = $1; }
    | loop_expr { $$ = $1; }
    | unnamed_prim { $$ = $1; }
    | named_prim { $$ = $1; }
    ;

/* ────────────────────────────────────────────────────────────────────────────
 * 容器表达式
 * ────────────────────────────────────────────────────────────────────────────
 */

/* Tuple */
tuple_expr
    : "(" expr "," ")" {
        /* 单元素 tuple: (expr,) */
        ASTNode list = create_expr_list();
        expr_list_add(list, $2);
        $$ = create_tuple_expr(list, true);
    }
    | "(" ref_expr "," ")" {
        /* 单元素 tuple with ref: (&x,) */
        ASTNode list = create_expr_list();
        expr_list_add(list, $2);
        $$ = create_tuple_expr(list, true);
    }
    | "(" expr "," expr_list ")" {
        /* 多元素 tuple: (a, b, c) */
        ASTNode list = create_expr_list();
        expr_list_add(list, $2);
        for (auto& elem : $4.children) {
            expr_list_add(list, std::move(elem));
        }
        $$ = create_tuple_expr(list, false);
    }
    | "(" ref_expr "," expr_list ")" {
        /* 多元素 tuple starting with ref: (&a, b, c) */
        ASTNode list = create_expr_list();
        expr_list_add(list, $2);
        for (auto& elem : $4.children) {
            expr_list_add(list, std::move(elem));
        }
        $$ = create_tuple_expr(list, false);
    }
    | "(" expr "," expr_list "," ")" {
        /* 多元素 tuple with trailing comma: (a, b,) */
        ASTNode list = create_expr_list();
        expr_list_add(list, $2);
        for (auto& elem : $4.children) {
            expr_list_add(list, std::move(elem));
        }
        $$ = create_tuple_expr(list, true);
    }
    | "(" ref_expr "," expr_list "," ")" {
        /* 多元素 tuple with trailing comma: (&a, b,) */
        ASTNode list = create_expr_list();
        expr_list_add(list, $2);
        for (auto& elem : $4.children) {
            expr_list_add(list, std::move(elem));
        }
        $$ = create_tuple_expr(list, true);
    }
    ;

/* List */
list_expr
    : "[" expr_list_opt "]" {
        $$ = create_list_expr($2);
    }
    ;

/* Dict */
dict_expr
    : "{" "}" {
        /* 空字典 */
        ASTNode empty = create_expr_list();
        $$ = create_dict_expr(empty);
    }
    | "{" dict_pair_list "}" {
        $$ = create_dict_expr($2);
    }
    | "{" dict_pair_list "," "}" {
        /* 尾随逗号 */
        $$ = create_dict_expr($2);
    }
    ;

dict_pair_list
    : dict_pair {
        ASTNode list = create_expr_list();
        expr_list_add(list, $1);
        $$ = list;
    }
    | dict_pair_list "," dict_pair {
        expr_list_add($1, $3);
        $$ = $1;
    }
    ;

dict_pair
    : expr ":" expr {
        $$ = create_dict_pair($1, $3);
    }
    | expr ":" ref_expr {
        $$ = create_dict_pair($1, $3);
    }
    ;

/* ────────────────────────────────────────────────────────────────────────────
 * Scope 和 Block
 * ────────────────────────────────────────────────────────────────────────────
 */

/* Scope: 普通作用域 */
scope_expr
    : "{" stmt_list "}" {
        /* 检查最后一个是否是表达式语句且没有分号 */
        bool use_tail = false;
        if (!$2.children.empty()) {
            // TODO: 需要在语法分析时记录是否有尾随分号
            // 暂时简单处理：假设如果最后是 expr_stmt 就 use_tail
            use_tail = true;
        }
        $$ = create_scope_expr($2, use_tail);
    }
    ;

/* Block: 用于 if/loop */
block_expr
    : "{" stmt_list "}" {
        bool use_tail = false;
        if (!$2.children.empty()) {
            use_tail = true;
        }
        $$ = create_block_expr($2, use_tail);
    }
    ;

/* ────────────────────────────────────────────────────────────────────────────
 * 控制流
 * ────────────────────────────────────────────────────────────────────────────
 */

/* If 表达式 */
if_expr
    : "if" expr block_expr if_else_chain {
        $$ = create_if_expr($2, $3, $4);
    }
    ;

if_else_chain
    : %empty {
        $$ = std::nullopt;
    }
    | "else" block_expr {
        $$ = $2;
    }
    | "else" if_expr {
        $$ = $2;
    }
    ;

/* Loop 表达式 */
loop_expr
    : "loop" label_opt block_expr {
        $$ = create_loop_expr($2, $3);
    }
    ;

label_opt
    : %empty {
        $$ = nullptr;
    }
    | "label" {
        $$ = $1;
    }
    ;

/* ────────────────────────────────────────────────────────────────────────────
 * Prim（函数）
 * ────────────────────────────────────────────────────────────────────────────
 */

/* 匿名 Prim */
unnamed_prim
    : "@" scope_expr {
        ASTNode empty_decorators = create_decorator_list();
        $$ = create_unnamed_prim(empty_decorators, $2);
    }
    | decorators "@" scope_expr {
        $$ = create_unnamed_prim($1, $3);
    }
    ;

/* 命名 Prim */
named_prim
    : "$" "identifier" "(" param_list_opt ")" type_hint_opt scope_expr {
        ASTNode empty_decorators = create_decorator_list();
        $$ = create_named_prim(empty_decorators, $2, $4, $6, $7);
    }
    | "$" "identifier" "(" param_list_opt ")" type_hint_opt "@" scope_expr {
        ASTNode empty_decorators = create_decorator_list();
        $$ = create_named_prim(empty_decorators, $2, $4, $6, $8);
    }
    | decorators "$" "identifier" "(" param_list_opt ")" type_hint_opt scope_expr {
        $$ = create_named_prim($1, $3, $5, $7, $8);
    }
    | decorators "$" "identifier" "(" param_list_opt ")" type_hint_opt "@" scope_expr {
        $$ = create_named_prim($1, $3, $5, $7, $9);
    }
    ;

/* 装饰器 */
decorators
    : "@" "identifier" {
        $$ = create_decorator_list();
        decorator_list_add($$, $2);
    }
    | decorators "@" "identifier" {
        decorator_list_add($1, $3);
        $$ = $1;
    }
    ;

/* 参数列表 */
param_list_opt
    : %empty {
        $$ = create_param_list();
    }
    | param_list {
        $$ = $1;
    }
    ;

param_list
    : param {
        $$ = create_param_list();
        param_list_add($$, $1);
    }
    | param_list "," param {
        param_list_add($1, $3);
        $$ = $1;
    }
    | param_list "," {
        /* 尾随逗号 */
        $$ = $1;
    }
    ;

param
    : "identifier" type_hint_opt {
        $$ = create_param($1, $2, false);
    }
    | "&" "identifier" type_hint_opt {
        $$ = create_param($2, $3, true);
    }
    ;

/* ────────────────────────────────────────────────────────────────────────────
 * 引用
 * ────────────────────────────────────────────────────────────────────────────
 */

ref_expr
    : "&" primary_expr {
        $$ = create_ref_expr($2);
    }
    ;

/* ────────────────────────────────────────────────────────────────────────────
 * 辅助规则
 * ────────────────────────────────────────────────────────────────────────────
 */

/* 表达式列表（可以混合 expr 和 ref_expr） */
expr_list_opt
    : %empty {
        $$ = create_expr_list();
    }
    | expr_list {
        $$ = $1;
    }
    ;

expr_list
    : expr {
        $$ = create_expr_list();
        expr_list_add($$, $1);
    }
    | ref_expr {
        $$ = create_expr_list();
        expr_list_add($$, $1);
    }
    | expr_list "," expr {
        expr_list_add($1, $3);
        $$ = $1;
    }
    | expr_list "," ref_expr {
        expr_list_add($1, $3);
        $$ = $1;
    }
    /* 注意: 不在 expr_list 层面处理尾随逗号，这由 tuple_expr 层面处理 */
    ;

/* 类型提示 */
type_hint_opt
    : %empty {
        $$ = std::nullopt;
    }
    | ":" type_hint {
        $$ = $2;
    }
    ;

type_hint
    : "identifier" {
        $$ = create_type_hint();
        type_hint_add($$, $1);
    }
    | type_hint "|" "identifier" {
        type_hint_add($1, $3);
        $$ = $1;
    }
    ;

%%

/* ============================================================================
 * 错误处理
 * ============================================================================
 */

void prim::detail::BisonParser::error(const location_type& loc, const std::string& msg) {
    // 将 Bison error 记录到 parser，使用正确的 location
    parser.add_error(prim::ParseError{
        prim::ParseErrorType::InvalidSyntax,
        prim::Location{loc.begin.line, loc.begin.column, 0},
        msg
    });
}
