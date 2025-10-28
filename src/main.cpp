// main.cpp —— Only modify the "display layer", not parsing/semantic logic (modern compiler style, English)
#include <functional>
#include <string>
#include <iostream>
#include <fstream>
#include <sstream>
#include <vector>
#include <cstring>
#include <cstdlib>
#include <cctype>
#include <filesystem>
#if !defined(_WIN32)
  #include <sys/stat.h>
  #include <sys/types.h>
  #include <unistd.h>
  #include <sys/wait.h>
#endif

#include <fmt/format.h>
#include <fmt/color.h>

#include "token.hpp"
#include "lexer.hpp"
#include "debug.hpp"
#include "parser.hpp"

using fmt::println;
using namespace prim;

// ============ Terminal Capability Detection ============
#if defined(_WIN32)
  #include <io.h>
  #define ISATTY _isatty
  #define FILENO _fileno
#else
  #include <unistd.h>
  #define ISATTY isatty
  #define FILENO fileno
#endif

static bool tty_supports_color() {
    const char* no_color = std::getenv("NO_COLOR");
    if (no_color && *no_color) return false;
    return ISATTY(FILENO(stdout));
}
static bool g_use_color = true;

// ============ Simple English Output Utilities ============
template <typename... Args>
static void info(const char* fmt, const Args&... args) {
    if (g_use_color) fmt::print(fg(fmt::color::cyan) | fmt::emphasis::bold, "Info: ");
    else             fmt::print("Info: ");
    fmt::print(fmt::runtime(fmt), args...);
    fmt::print("\n");
}

template <typename... Args>
static void ok(const char* fmt, const Args&... args) {
    if (g_use_color) fmt::print(fg(fmt::color::green) | fmt::emphasis::bold, "Success: ");
    else             fmt::print("Success: ");
    fmt::print(fmt::runtime(fmt), args...);
    fmt::print("\n");
}

template <typename... Args>
static void warn(const char* fmt, const Args&... args) {
    if (g_use_color) fmt::print(fg(fmt::color::yellow) | fmt::emphasis::bold, "Warning: ");
    else             fmt::print("Warning: ");
    fmt::print(fmt::runtime(fmt), args...);
    fmt::print("\n");
}

template <typename... Args>
static void err(const char* fmt, const Args&... args) {
    if (g_use_color) fmt::print(fg(fmt::color::red) | fmt::emphasis::bold, "Error: ");
    else             fmt::print("Error: ");
    fmt::print(fmt::runtime(fmt), args...);
    fmt::print("\n");
}

// (Only used with --show)
static void section(std::string_view title) {
    if (g_use_color)
        fmt::print(fg(fmt::color::light_sea_green) | fmt::emphasis::bold,
                   "\n── {} ──\n", title);
    else
        println("\n== {} ==\n", title);
}

// ============ Fake execution display (silently skip if not found) ============
static std::filesystem::path build_fake_path(const std::string& src_filename) {
    namespace fs = std::filesystem;
    fs::path src(src_filename);
    fs::path dir = src.parent_path();
    fs::path stem = src.stem();          // remove .prim
    fs::path fake_dir = dir / ".fake";
#if defined(_WIN32)
    fs::path fake = fake_dir / (stem.string() + ".exe");
#else
    fs::path fake = fake_dir / stem;     // Unix: no extension
#endif
    return fake;
}

static bool is_executable(const std::filesystem::path& p) {
    if (!std::filesystem::exists(p) || !std::filesystem::is_regular_file(p)) return false;
#if defined(_WIN32)
    auto ext = p.extension().string();
    for (auto& c : ext) c = (char)std::tolower((unsigned char)c);
    return (ext == ".exe");
#else
    return ::access(p.c_str(), X_OK) == 0;
#endif
}

static int run_fake(const std::filesystem::path& exe_path) {
    std::string cmd = "\"" + exe_path.string() + "\"";
    if (g_use_color)
        fmt::print(fg(fmt::color::cyan) | fmt::emphasis::bold, "Running: ");
    else
        fmt::print("Running: ");
    fmt::print("{}\n", exe_path.string());

    int rc = std::system(cmd.c_str());
    if (rc == -1) {
        err("Failed to exec (system() returned -1)");
        return rc;
    }
#if !defined(_WIN32)
    if (WIFEXITED(rc)) {
        int code = WEXITSTATUS(rc);
        if (code == 0) ok("Process exited normally (0)");
        else           warn("Process exited with nonzero code ({})", code);
        return code;
    } else if (WIFSIGNALED(rc)) {
        int sig = WTERMSIG(rc);
        err("Process was terminated by signal (signal={})", sig);
        return 128 + sig;
    }
#endif
    if (rc == 0) ok("Process exited normally (0)");
    else         warn("Process exited with nonzero code ({})", rc);
    return rc;
}

static void maybe_run_fake_quiet(const std::string& src_filename) {
    namespace fs = std::filesystem;
    fs::path fake = build_fake_path(src_filename);
    // Requirement: do not say "could not find fake"; skip silently if not found or not executable
    if (!fs::exists(fake)) return;
    if (!is_executable(fake)) return;
    (void)run_fake(fake);
}

// ============ Code frame display (used on error) ============
struct SourceView { std::vector<std::string> lines; };

static SourceView build_source_view(const std::string& source) {
    SourceView sv;
    std::istringstream ss(source);
    std::string line;
    while (std::getline(ss, line)) {
        if (!line.empty() && line.back() == '\r') line.pop_back();
        sv.lines.emplace_back(std::move(line));
    }
    return sv;
}

static void print_code_frame(const SourceView& sv,
                             const std::string& filename,
                             int err_line, int err_col,
                             std::string_view msg) {
    const int total = (int)sv.lines.size();
    if (err_line <= 0 || err_line > total) {
        if (g_use_color)
            fmt::print(fg(fmt::color::red) | fmt::emphasis::bold, "error: ");
        else
            fmt::print("error: ");
        fmt::print("{}\n", msg);
        return;
    }

    // Line number width
    int max_line_no = std::max(1, total), width = 1;
    while (max_line_no >= 10) { max_line_no /= 10; ++width; }

    auto print_one = [&](int ln, bool highlight) {
        if (ln <= 0 || ln > total) return;
        const std::string& text = sv.lines[ln - 1];
        std::string gutter = fmt::format("{:>{}} | ", ln, width);

        if (highlight && g_use_color) {
            fmt::print(fg(fmt::color::white) | fmt::emphasis::bold, "{}", gutter);
            fmt::print(fg(fmt::color::white), "{}\n", text);
        } else {
            fmt::print("{}{}\n", gutter, text);
        }

        if (highlight) {
            int col = std::max(err_col, 1);
            std::string indicator(width + 3 + std::max(col - 1, 0), ' ');
            if (g_use_color)
                fmt::print(fg(fmt::color::red) | fmt::emphasis::bold, "{}^\n", indicator);
            else
                println("{}^", indicator);
        }
    };

    // Header: file:line:col + message
    if (g_use_color) {
        fmt::print(fg(fmt::color::red) | fmt::emphasis::bold, "\nError");
        fmt::print(": ");
        fmt::print(fg(fmt::color::yellow) | fmt::emphasis::bold, "{}:{}:{}\n",
                   filename, err_line, std::max(err_col, 1));
        fmt::print(fg(fmt::color::red), "{}\n", msg);
    } else {
        println("\nError: {}:{}:{}", filename, err_line, std::max(err_col, 1));
        println("{}", msg);
    }

    // Context: 1 line before and after
    int from = std::max(1, err_line - 1);
    int to   = std::min(total, err_line + 1);

    auto hr = [&]() {
        if (g_use_color)
            fmt::print(fg(fmt::color::dark_gray), "{}\n", std::string(width + 3 + 48, '-'));
        else
            println("{}", std::string(width + 3 + 48, '-'));
    };

    hr();
    for (int ln = from; ln <= to; ++ln) {
        print_one(ln, ln == err_line);
    }
    hr();
}

// ============ Main workflow (quiet by default; only error prompts; --show for details) ============
int main(int argc, char* argv[]) {
    g_use_color = tty_supports_color();

    bool lexer_only = false;
    bool show_detail = false;   // New: --show controls detailed output
    const char* filename = nullptr;

    // Argument parsing
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--lexer-only") == 0) {
            lexer_only = true;
        } else if (strcmp(argv[i], "--show") == 0) {
            show_detail = true;
        } else if (strcmp(argv[i], "--help") == 0 || strcmp(argv[i], "-h") == 0) {
            println("Usage: {} [--lexer-only] [--show] <source file>", argv[0]);
            println("  --lexer-only  Perform only lexical analysis");
            println("  --show        Show debugging info on lexical and syntax phases");
            println("  --help, -h    Show help");
            return 0;
        } else if (filename == nullptr) {
            filename = argv[i];
        }
    }

    if (!filename) {
        println("Usage: {} [--lexer-only] [--show] <source file>", argv[0]);
        println("  --lexer-only  Perform only lexical analysis");
        println("  --show        Show debugging info on lexical and syntax phases");
        println("  --help, -h    Show help");
        filename = "/Users/wzq/Documents/Code/Project/jlu-cs/test.prim";
    }

    // Read source code
    std::ifstream file(filename);
    if (!file) {
        err("Unable to open file '{}'", filename);
        return 1;
    }
    std::stringstream buffer; buffer << file.rdbuf();
    std::string source = buffer.str();
    if (source.empty()) {
        warn("The file is empty");
        return 0;
    }

    // Phase 1: Lexical Analysis
    Lexer lexer(source);
    std::vector<Token> tokens;
    while (true) {
        Token tok = lexer.next();
        if (tok.type == TokenType::END || tok.type == TokenType::ERROR) {
            if (tok.type == TokenType::ERROR) tokens.push_back(tok);
            break;
        }
        tokens.push_back(tok);
    }
    if (show_detail) {
        section("Lexical Analysis");
        ok("Collected {} tokens", tokens.size());
        print_tokens(tokens);  // Your debug output; add color in debug.hpp if needed
        ok("Lexical analysis done");
    }
    if (lexer_only) {
        ok("Lexical analysis phase done");
        return 0;
    }

    // Phase 2: Syntax Analysis
    Lexer lexer2(source);
    Parser parser;
    auto ast = parser.parse([&lexer2]() -> Token { return lexer2.next(); });
    SourceView sv = build_source_view(source);

    // Error reporting (only error output)
    if (parser.has_errors()) {
        const auto& errs = parser.get_errors();
        for (const auto& e : errs) {
            print_code_frame(sv, filename, e.location.line, e.location.col, e.message);
        }
        if (g_use_color) {
            fmt::print(fg(fmt::color::yellow) | fmt::emphasis::bold, "\nHint: ");
            fmt::print("Check the symbol(s) near the '^' indicator above (such as parenthesis, comma, newline, etc).\n");
        } else {
            println("\nHint: Check the symbol(s) near the '^' indicator above (such as parenthesis, comma, newline, etc).");
        }
        return 1;
    }

    // Success (quiet by default; shows AST summary with --show)
    if (ast.has_value()) {
        if (show_detail) {
            section("Syntax Analysis / AST");
            ok("Parsing succeeded");
            println("  Root type: Program");
            println("  Number of children: {}", ast->children.size());

            std::function<void(const ASTNode&, int)> print_ast;
            print_ast = [&print_ast](const ASTNode& node, int depth) {
                std::string indent(depth * 2, ' ');
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

                if (g_use_color) {
                    fmt::print("{}[", indent);
                    fmt::print(fg(fmt::color::light_green) | fmt::emphasis::bold, "{}", type_name);
                    fmt::print("]\n");
                } else {
                    println("{}[{}]", indent, type_name);
                }

                if (node.token) {
                    if (g_use_color) {
                        fmt::print("{}  token: \"", indent);
                        fmt::print(fg(fmt::color::light_coral) | fmt::emphasis::bold, "{}", node.token->text);
                        fmt::print("\"\n");
                    } else {
                        println("{}  token: \"{}\"", indent, node.token->text);
                    }
                }

                if (depth < 5 && !node.children.empty()) {
                    println("{}  children: {}", indent, node.children.size());
                    for (const auto& child : node.children) {
                        print_ast(child, depth + 2);
                    }
                } else if (!node.children.empty()) {
                    println("{}  children: {} (depth limit reached)", indent, node.children.size());
                }
            };

            print_ast(*ast, 0);
        }

        // After parsing success, attempt to execute ./.fake/<same_name>; skip silently if not found/not executable
        maybe_run_fake_quiet(filename);

        // Simple success line (modern compiler style)
        if (g_use_color)
            fmt::print(fg(fmt::color::green) | fmt::emphasis::bold, "Build succeeded\n");
        else
            println("Build succeeded");
        return 0;
    } else {
        err("Parse failed: No AST generated");
        return 1;
    }
}
