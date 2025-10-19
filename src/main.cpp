#include <string>
#include <fmt/format.h>

#include "lexer.hpp"

using fmt::print, fmt::println;


int main() {
    std::string input = R"(
@struct $Point(x: i32, y: i32) @{
    let x = x;     // 值引入
    let y = y;

    // 运算符重载：闭包方法表里注册 "+"
    $+(other: Point): Point {
        Point(&x + other.x, &y + other.y)
    }
}

$scale(p: Point, k: i32): Point {
    // 只读值：p 是值绑定，修改不影响调用者
    Point(p.x * k, p.y * k)
}

let px = 1; let py = 2;
let p1 = Point(px, py);
let p2 = Point(3, 4);
let p3 = p1 + p2;       // 通过方法表派发

let ans = $main() {
    let &p1;            // 引入外层 p1 的槽为引用
    p1 = Point(10, 20); // 写回外层槽
    p3
};

ans();                  // 返回 p3
p1.x;                   // 10（已被上面修改）
)";
    Lexer lexer(input);
    Token token;
    while (token.kind != TokenKind::END) {
        token = lexer.next();
        println("{}", static_cast<int>(token.kind));
    }
    return 0;
}