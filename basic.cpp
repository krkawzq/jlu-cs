#include <iostream>
#include <string>
#include <vector>
#include <unordered_map>
#include <memory>
#include <functional>
#include <sstream>
#include <cctype>
#include <type_traits>
#include <algorithm> // for std::all_of

// --------------------- 全局环境 ---------------------
std::unordered_map<std::string, std::string> g_env;
static std::vector<std::vector<std::string>> g_call_args_stack;

// --------------------- AST 节点定义 ---------------------

enum class NodeType {
    Comment,
    Assign,
    Function,
    Execute,
    If,
    While,
    Test,   // [[ ... ]]
    Arith   // (( ... ))
};

// 前置声明
struct Node;
struct Frame;

// ---------- 基础节点 ----------
struct AssignNode {
    std::string variable;
    std::string value;
};

struct ExecuteNode {
    std::string command;
    std::vector<std::string> params;
};

struct FunctionNode {
    std::string name;
    std::unique_ptr<Frame> body;
};

struct TestNode {
    std::string expr; // [[ ... ]] 内部原始表达式
};

struct ArithNode {
    std::string expr; // (( ... )) 内部原始表达式
};

// ---------- 控制结构 ----------
struct IfBranch {
    std::variant<TestNode, ArithNode, ExecuteNode> cond;
    std::unique_ptr<Frame> body;
};

struct IfNode {
    std::vector<IfBranch> branches;
    bool has_else = false;
    std::unique_ptr<Frame> else_body;
};

struct WhileNode {
    std::variant<TestNode, ArithNode, ExecuteNode> cond;
    std::unique_ptr<Frame> body;
};

// ---------- 通用 Node ----------
struct Node {
    NodeType type;
    int line = 0;

    std::variant<
        std::monostate,
        AssignNode,
        ExecuteNode,
        FunctionNode,
        IfNode,
        WhileNode,
        TestNode,
        ArithNode
    > content;
};

struct Frame {
    std::vector<std::unique_ptr<Node>> nodes;
};

static std::unordered_map<std::string, std::unique_ptr<Frame>> g_functions;

// 循环控制状态
struct ExecControl {
    bool should_break = false;
    bool should_continue = false;
};

// --------------------- 词法/语法 ---------------------

static std::string trim_copy(const std::string& s) {
    size_t i = 0, j = s.size();
    while (i < j && std::isspace(static_cast<unsigned char>(s[i]))) i++;
    while (j > i && std::isspace(static_cast<unsigned char>(s[j-1]))) j--;
    return s.substr(i, j - i);
}

static bool is_blank_or_comment(const std::string& line) {
    bool in_single = false, in_double = false;
    for (size_t i = 0; i < line.size(); i++) {
        char c = line[i];
        if (c == '\'' && !in_double) in_single = !in_single;
        else if (c == '"' && !in_single) in_double = !in_double;
        else if (c == '#' && !in_single && !in_double) {
            return trim_copy(line.substr(0, i)).empty();
        }
    }
    return trim_copy(line).empty();
}

static std::vector<std::string> tokenize(const std::string& line) {
    std::vector<std::string> tokens;
    std::string cur;
    bool in_single = false, in_double = false;
    bool in_test = false, in_arith = false;
    for (size_t i = 0; i < line.size(); i++) {
        char c = line[i];
        // 评论
        if (!in_single && !in_double && !in_test && !in_arith && c == '#') break;
        // 进入/退出 [[ ... ]]
        if (!in_single && !in_double && !in_arith && i + 1 < line.size() && line[i] == '[' && line[i+1] == '[') {
            if (!cur.empty()) { tokens.push_back(cur); cur.clear(); }
            in_test = true; i++;
            cur = "[[";
            continue;
        }
        if (in_test && i + 1 < line.size() && line[i] == ']' && line[i+1] == ']') {
            cur += "]]"; i++;
            tokens.push_back(cur); cur.clear(); in_test = false; continue;
        }
        // 进入/退出 (( ... ))
        if (!in_single && !in_double && !in_test && i + 1 < line.size() && line[i] == '(' && line[i+1] == '(') {
            if (!cur.empty()) { tokens.push_back(cur); cur.clear(); }
            in_arith = true; i++;
            cur = "((";
            continue;
        }
        if (in_arith && i + 1 < line.size() && line[i] == ')' && line[i+1] == ')') {
            cur += "))"; i++;
            tokens.push_back(cur); cur.clear(); in_arith = false; continue;
        }
        // 引号
        if (!in_double && c == '\'' && !in_test && !in_arith) { in_single = !in_single; cur.push_back(c); continue; }
        if (!in_single && c == '"' && !in_test && !in_arith) { in_double = !in_double; cur.push_back(c); continue; }
        // 分隔
        if (!in_single && !in_double && !in_test && !in_arith && std::isspace(static_cast<unsigned char>(c))) {
            if (!cur.empty()) { tokens.push_back(cur); cur.clear(); }
            continue;
        }
        cur.push_back(c);
    }
    if (!cur.empty()) tokens.push_back(cur);
    return tokens;
}

enum class BlockKind { IF, WHILE, BRACE };
struct BlockFrame {
    BlockKind kind;
    Node* node;
};

static bool is_test_token(const std::string& t) { return t.rfind("[[", 0) == 0 && t.size() >= 4 && t.substr(t.size()-2) == "]]"; }
static bool is_arith_token(const std::string& t) { return t.rfind("((", 0) == 0 && t.size() >= 4 && t.substr(t.size()-2) == "))"; }
static std::string unwrap_group(const std::string& t) {
    if (is_test_token(t)) return trim_copy(t.substr(2, t.size()-4));
    if (is_arith_token(t)) return trim_copy(t.substr(2, t.size()-4));
    return t;
}

static std::variant<TestNode, ArithNode, ExecuteNode> parse_condition(const std::vector<std::string>& tokens, size_t start_idx) {
    if (start_idx < tokens.size() && is_test_token(tokens[start_idx])) {
        return TestNode{unwrap_group(tokens[start_idx])};
    }
    if (start_idx < tokens.size() && is_arith_token(tokens[start_idx])) {
        return ArithNode{unwrap_group(tokens[start_idx])};
    }
    ExecuteNode ex;
    if (start_idx < tokens.size()) {
        ex.command = tokens[start_idx];
        for (size_t i = start_idx + 1; i < tokens.size(); i++) ex.params.push_back(tokens[i]);
    }
    return ex;
}

static std::unique_ptr<Frame> parse_lines(const std::vector<std::string>& lines) {
    auto root = std::make_unique<Frame>();
    std::vector<Frame*> frame_stack;
    frame_stack.push_back(root.get());
    std::vector<BlockFrame> block_stack;
    Frame* func_frame = nullptr;

    for (const auto& raw : lines) {
        if (is_blank_or_comment(raw)) continue;
        auto tokens = tokenize(raw);
        if (tokens.empty()) continue;
        const std::string& t0 = tokens[0];

        // function name { ... }
        if (t0 == "function" && tokens.size() >= 2) {
            std::string fname = tokens[1];
            auto fn_frame = std::make_unique<Frame>();
            func_frame = fn_frame.get();
            g_functions[fname] = std::move(fn_frame);
            continue;
        }
        if (t0 == "{") { continue; }
        if (t0 == "}") { func_frame = nullptr; continue; }

        if (t0 == "if") {
            auto node = std::make_unique<Node>();
            node->type = NodeType::If;
            IfNode ifn;
            IfBranch br;
            br.cond = parse_condition(tokens, 1);
            br.body = std::make_unique<Frame>();
            ifn.branches.push_back(std::move(br));
            node->content = std::move(ifn);
            auto* n_ptr = node.get();
            (func_frame ? func_frame : frame_stack.back())->nodes.push_back(std::move(node));
            block_stack.push_back({BlockKind::IF, n_ptr});
            continue;
        }

        if (t0 == "elif") {
            if (block_stack.empty() || block_stack.back().kind != BlockKind::IF) continue;
            auto* if_node = &std::get<IfNode>(block_stack.back().node->content);
            IfBranch br;
            br.cond = parse_condition(tokens, 1);
            br.body = std::make_unique<Frame>();
            if_node->branches.push_back(std::move(br));
            continue;
        }

        if (t0 == "else") {
            if (block_stack.empty() || block_stack.back().kind != BlockKind::IF) continue;
            auto* if_node = &std::get<IfNode>(block_stack.back().node->content);
            if_node->has_else = true;
            if_node->else_body = std::make_unique<Frame>();
            continue;
        }

        if (t0 == "then") {
            // no-op in this simplified parser
            continue;
        }

        if (t0 == "fi") {
            if (!block_stack.empty() && block_stack.back().kind == BlockKind::IF) block_stack.pop_back();
            continue;
        }

        if (t0 == "while") {
            auto node = std::make_unique<Node>();
            node->type = NodeType::While;
            WhileNode wn;
            wn.cond = parse_condition(tokens, 1);
            wn.body = std::make_unique<Frame>();
            node->content = std::move(wn);
            auto* n_ptr = node.get();
            (func_frame ? func_frame : frame_stack.back())->nodes.push_back(std::move(node));
            block_stack.push_back({BlockKind::WHILE, n_ptr});
            continue;
        }

        if (t0 == "do") {
            // no-op
            continue;
        }

        if (t0 == "done") {
            if (!block_stack.empty() && block_stack.back().kind == BlockKind::WHILE) block_stack.pop_back();
            continue;
        }

        // 赋值：a = b  或 a=b
        bool assigned = false;
        if (tokens.size() == 1) {
            const std::string& kv = tokens[0];
            size_t pos = kv.find('=');
            if (pos != std::string::npos && pos > 0) {
                auto node = std::make_unique<Node>();
                node->type = NodeType::Assign;
                node->content = AssignNode{ kv.substr(0, pos), kv.substr(pos + 1) };
                if (!block_stack.empty()) {
                    if (block_stack.back().kind == BlockKind::IF) {
                        auto& ifn = std::get<IfNode>(block_stack.back().node->content);
                        Frame* target = ifn.has_else ? ifn.else_body.get() : ifn.branches.back().body.get();
                        target->nodes.push_back(std::move(node));
                    } else if (block_stack.back().kind == BlockKind::WHILE) {
                        auto& wn = std::get<WhileNode>(block_stack.back().node->content);
                        wn.body->nodes.push_back(std::move(node));
                    }
                } else {
                    (func_frame ? func_frame : frame_stack.back())->nodes.push_back(std::move(node));
                }
                assigned = true;
            }
        }
        if (!assigned && tokens.size() >= 3 && tokens[1] == "=") {
            auto node = std::make_unique<Node>();
            node->type = NodeType::Assign;
            node->content = AssignNode{tokens[0], tokens[2]};
            if (!block_stack.empty()) {
                if (block_stack.back().kind == BlockKind::IF) {
                    auto& ifn = std::get<IfNode>(block_stack.back().node->content);
                    Frame* target = ifn.has_else ? ifn.else_body.get() : ifn.branches.back().body.get();
                    target->nodes.push_back(std::move(node));
                } else if (block_stack.back().kind == BlockKind::WHILE) {
                    auto& wn = std::get<WhileNode>(block_stack.back().node->content);
                    wn.body->nodes.push_back(std::move(node));
                }
            } else {
                (func_frame ? func_frame : frame_stack.back())->nodes.push_back(std::move(node));
            }
            continue;
        }
        if (assigned) continue;

        // 执行命令或 [[ ]] / (( ))
        auto node = std::make_unique<Node>();
        if (is_test_token(t0)) {
            node->type = NodeType::Test;
            node->content = TestNode{unwrap_group(t0)};
        } else if (is_arith_token(t0)) {
            node->type = NodeType::Arith;
            node->content = ArithNode{unwrap_group(t0)};
        } else {
            node->type = NodeType::Execute;
            ExecuteNode ex;
            ex.command = tokens[0];
            for (size_t i = 1; i < tokens.size(); i++) ex.params.push_back(tokens[i]);
            node->content = std::move(ex);
        }
        if (!block_stack.empty()) {
            if (block_stack.back().kind == BlockKind::IF) {
                auto& ifn = std::get<IfNode>(block_stack.back().node->content);
                Frame* target = ifn.has_else ? ifn.else_body.get() : ifn.branches.back().body.get();
                target->nodes.push_back(std::move(node));
            } else if (block_stack.back().kind == BlockKind::WHILE) {
                auto& wn = std::get<WhileNode>(block_stack.back().node->content);
                wn.body->nodes.push_back(std::move(node));
            }
        } else {
            (func_frame ? func_frame : frame_stack.back())->nodes.push_back(std::move(node));
        }
    }
    return root;
}

static bool is_all_digits(const std::string& s) {
    return !s.empty() && std::all_of(s.begin(), s.end(), [](unsigned char ch){ return std::isdigit(ch) != 0; });
}

// 跟踪 if/while 与 { }，用于 REPL 缓冲是否需要 flush
static bool update_block_balance(const std::vector<std::string>& tokens, std::vector<BlockKind>& st) {
    if (tokens.empty()) return !st.empty();
    for (const auto& t0 : tokens) {
        if (t0 == "if") st.push_back(BlockKind::IF);
        else if (t0 == "fi") { 
            if (!st.empty()) {
                // 从后往前找最近的 IF
                for (int i = (int)st.size()-1; i >= 0; --i) if (st[i] == BlockKind::IF) { st.erase(st.begin()+i); break; }
            }
        }
        else if (t0 == "while") st.push_back(BlockKind::WHILE);
        else if (t0 == "done") {
            if (!st.empty()) {
                for (int i = (int)st.size()-1; i >= 0; --i) if (st[i] == BlockKind::WHILE) { st.erase(st.begin()+i); break; }
            }
        }
        else if (t0 == "{") st.push_back(BlockKind::BRACE);
        else if (t0 == "}") {
            if (!st.empty()) {
                for (int i = (int)st.size()-1; i >= 0; --i) if (st[i] == BlockKind::BRACE) { st.erase(st.begin()+i); break; }
            }
        }
    }
    return !st.empty();
}

// --------------------- 内建命令 ---------------------

using BuiltinFn = std::function<int(const std::vector<std::string>&)>;

int cmd_echo(const std::vector<std::string>& args) {
    for (size_t i = 1; i < args.size(); i++) {
        if (i > 1) std::cout << " ";
        std::cout << args[i];
    }
    std::cout << "\n";
    return 0;
}

int cmd_dbg(const std::vector<std::string>&) {
    std::cout << "[dbg] not implemented yet\n";
    return 0;
}

int cmd_expand(const std::vector<std::string>& args) {
    for (auto& arg : args) {
        if (!arg.empty() && arg[0] == '$') {
            const char* val = getenv(arg.substr(1).c_str());
            std::cout << (val ? val : "") << " ";
        } else {
            std::cout << arg << " ";
        }
    }
    std::cout << "\n";
    return 0;
}

int cmd_true(const std::vector<std::string>&) { return 0; }
int cmd_false(const std::vector<std::string>&) { return 1; }
int cmd_set(const std::vector<std::string>&) {
    for (auto& [k,v] : g_env) {
        std::cout << k << "=" << v << "\n";
    }
    return 0;
}

// 自增：inc VAR [DELTA]
int cmd_inc(const std::vector<std::string>& args) {
    if (args.size() < 2) return 1;
    std::string key = args[1];
    int delta = 1;
    if (args.size() >= 3) {
        try { delta = std::stoi(args[2]); } catch (...) { return 1; }
    }
    int cur = 0;
    auto it = g_env.find(key);
    if (it != g_env.end()) {
        try { cur = std::stoi(it->second); } catch (...) { cur = 0; }
    }
    g_env[key] = std::to_string(cur + delta);
    return 0;
}

// 比较小于：lt A B  -> return 0 if A < B else 1
int cmd_lt(const std::vector<std::string>& args) {
    if (args.size() != 3) return 1;
    try {
        long long a = std::stoll(args[1]);
        long long b = std::stoll(args[2]);
        return (a < b) ? 0 : 1;
    } catch (...) {
        return 1;
    }
}

// --------------------- 内建表 ---------------------

std::unordered_map<std::string, BuiltinFn> builtins = {
    {"echo",   cmd_echo},
    {"dbg",    cmd_dbg},
    {"expand", cmd_expand},
    {"true",   cmd_true},
    {"false",  cmd_false},
    {"set",    cmd_set},
    {"inc",    cmd_inc},
    {"lt",     cmd_lt},
};

// --------------------- 内建表达式 [[ / (( )) ---------------------

int builtin_test(const std::string& expr) {
    std::istringstream iss(expr);
    std::vector<std::string> tokens;
    std::string tok;
    while (iss >> tok) tokens.push_back(tok);

    if (tokens.empty()) return 1;

    if (tokens.size() == 1) {
        return tokens[0].empty() ? 1 : 0;
    }
    if (tokens.size() == 3) {
        auto a = tokens[0], op = tokens[1], b = tokens[2];
        if (op == "=") return (a == b ? 0 : 1);
        if (op == "!=") return (a != b ? 0 : 1);
        try {
            long long ai = std::stoll(a), bi = std::stoll(b);
            if (op == "-eq") return (ai == bi ? 0 : 1);
            if (op == "-ne") return (ai != bi ? 0 : 1);
            if (op == "-lt") return (ai < bi ? 0 : 1);
            if (op == "-le") return (ai <= bi ? 0 : 1);
            if (op == "-gt") return (ai > bi ? 0 : 1);
            if (op == "-ge") return (ai >= bi ? 0 : 1);
        } catch (...) {
            return 1;
        }
    }
    return 1;
}

int builtin_arith(const std::string& expr) {
    try {
        int val = std::stoi(expr);
        return (val != 0) ? 0 : 1;
    } catch (...) {
        return 1;
    }
}

// --------------------- 执行器 ---------------------

// 前向声明，供 exec_command 使用
int exec_frame(Frame& frame);
int exec_frame_ctrl(Frame& frame, ExecControl& ctrl);

int exec_command(const std::vector<std::string>& args) {
    if (args.empty()) return 0;
    // 用户函数
    auto fit = g_functions.find(args[0]);
    if (fit != g_functions.end()) {
        std::vector<std::string> call_args;
        for (size_t i = 1; i < args.size(); i++) call_args.push_back(args[i]);
        g_call_args_stack.push_back(call_args);
        int st = exec_frame(*fit->second);
        g_call_args_stack.pop_back();
        return st;
    }
    auto it = builtins.find(args[0]);
    if (it == builtins.end()) {
        std::cerr << "Unknown command: " << args[0] << "\n";
        return 127;
    }
    return it->second(args);
}


static std::string expand_one_arg(const std::string& s) {
    if (!s.empty() && s[0] == '$') {
        std::string key = s.substr(1);
        if (key == "@") {
            if (!g_call_args_stack.empty()) {
                std::string joined;
                const auto& arr = g_call_args_stack.back();
                for (size_t i = 0; i < arr.size(); i++) {
                    if (i) joined += ' ';
                    joined += arr[i];
                }
                return joined;
            }
            return std::string();
        }
        if (is_all_digits(key)) {
            size_t idx = std::stoul(key);
            if (!g_call_args_stack.empty() && idx >= 1 && idx <= g_call_args_stack.back().size())
                return g_call_args_stack.back()[idx - 1];
            return std::string();
        }
        auto it = g_env.find(key);
        if (it != g_env.end()) return it->second;
        const char* sys = getenv(key.c_str());
        return sys ? std::string(sys) : std::string();
    }
    return s;
}

int exec_node(Node& node) {
    switch (node.type) {
    case NodeType::Assign: {
        auto& n = std::get<AssignNode>(node.content);
        g_env[n.variable] = n.value;
        return 0;
    }
    case NodeType::Execute: {
        auto& n = std::get<ExecuteNode>(node.content);
        std::vector<std::string> args;
        args.push_back(n.command);
        for (auto& p : n.params) args.push_back(expand_one_arg(p));
        return exec_command(args);
    }
    case NodeType::Test: {
        auto& n = std::get<TestNode>(node.content);
        return builtin_test(n.expr);
    }
    case NodeType::Arith: {
        auto& n = std::get<ArithNode>(node.content);
        return builtin_arith(n.expr);
    }
    case NodeType::Function: {
        return 0;
    }
    case NodeType::If: {
        auto& n = std::get<IfNode>(node.content);
        for (auto& br : n.branches) {
            int status = 1;
            std::visit([&](auto& cond) {
                using T = std::decay_t<decltype(cond)>;
                if constexpr (std::is_same_v<T, TestNode>)
                    status = builtin_test(cond.expr);
                else if constexpr (std::is_same_v<T, ArithNode>)
                    status = builtin_arith(cond.expr);
                else if constexpr (std::is_same_v<T, ExecuteNode>) {
                    std::vector<std::string> args{cond.command};
                    for (auto& p : cond.params) args.push_back(expand_one_arg(p));
                    status = exec_command(args);
                }
            }, br.cond);
            if (status == 0) return exec_frame(*br.body);
        }
        if (n.has_else) return exec_frame(*n.else_body);
        return 0;
    }
    case NodeType::While: {
        auto& n = std::get<WhileNode>(node.content);
        while (true) {
            int status = 1;
            std::visit([&](auto& cond) {
                using T = std::decay_t<decltype(cond)>;
                if constexpr (std::is_same_v<T, TestNode>)
                    status = builtin_test(cond.expr);
                else if constexpr (std::is_same_v<T, ArithNode>)
                    status = builtin_arith(cond.expr);
                else if constexpr (std::is_same_v<T, ExecuteNode>) {
                    std::vector<std::string> args{cond.command};
                    for (auto& p : cond.params) args.push_back(expand_one_arg(p));
                    status = exec_command(args);
                }
            }, n.cond);
            if (status != 0) break;
            ExecControl ctrl;
            exec_frame_ctrl(*n.body, ctrl);
            if (ctrl.should_break) break;
            if (ctrl.should_continue) continue;
        }
        return 0;
    }
    default:
        return 0;
    }
}

int exec_frame(Frame& frame) {
    int status = 0;
    for (auto& n : frame.nodes) {
        status = exec_node(*n);
    }
    return status;
}

int exec_frame_ctrl(Frame& frame, ExecControl& ctrl) {
    int status = 0;
    for (auto& n : frame.nodes) {
        if (n->type == NodeType::Execute) {
            auto& ex = std::get<ExecuteNode>(n->content);
            if (ex.command == "break") { ctrl.should_break = true; return 0; }
            if (ex.command == "continue") { ctrl.should_continue = true; return 0; }
        }
        status = exec_node(*n);
        if (ctrl.should_break || ctrl.should_continue) return status;
    }
    return status;
}

// --------------------- 测试 ---------------------

int main() {
    std::vector<std::string> buffer;
    std::vector<BlockKind> blk;
    auto flush_buffer = [&](){
        auto frame = parse_lines(buffer);
        exec_frame(*frame);
        buffer.clear();
    };

    std::string line;
    while (true) {
        std::cout << (blk.empty() ? "> " : "... ");
        if (!std::getline(std::cin, line)) break;
        if (is_blank_or_comment(line)) continue;
        auto tokens = tokenize(line);
        bool open = update_block_balance(tokens, blk);
        buffer.push_back(line);
        if (!open) flush_buffer();
    }
    if (!buffer.empty()) flush_buffer();
    return 0;
}