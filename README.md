# jlu-cs
课程设计

## 简单解释器

### 1. 语法规则

指令 + 参数
echo $a
echo "hello, world"
echo "hello, ${a}"

赋值语句（字符串统一处理）
a = 1
a = abc

控制语句（if else elif while)

if cond; then; fi

if cond
then
fi

while COMMANDS; do
    BODY
done


if COMMANDS; then
    BODY
elif COMMANDS; then
    BODY
elif COMMANDS; then
    BODY
...
else
    BODY
fi

### 扩展语法

```
$a
$(())
$[[]]
```

### 引号

''字面量，转义
"允许展开"

### 注释

#来注释

### 退出码

任何语句都是一个命令，命令都有退出码

扩展语法是一种字符串替换宏

if 等控制语句通过控制码来作为条件

### 内置命令

echo 等

### 调试

dbg()使用命令的方式来断电（相当于进入一个特殊的 readline 函数，可以查询环境变量）

### 多行判定

简单

### 错误处理

不做处理，直接崩溃

### 函数

使用

function func {
    echo $1
}

来构建函数

$1是实参

$@是参数列表

参数的解析是自动的按空格划分