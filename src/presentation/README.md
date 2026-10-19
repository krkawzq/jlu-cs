# Prim 交互式课堂展示

主讲 27 页、附录 5 页，四幕循序展开：同一个汇总器的三种回答 → 一切皆闭包（槽、捕获、对象）→ 内存、手动 JIT 与 `std.inspect` 看到的机器码 → 矩阵乘法、订单处理、贪吃蛇。每个概念页之后紧跟一页**现场运行**：整页左侧是编辑器，右侧是真实执行结果。文案与版式都在 [class-presentation.md](../../docs/class-presentation.md)。

网页是唯一排版来源。离线 PPTX 由 Chromium 逐页截图生成，分步内容全部展开，保留实际测量表、现场运行页的输出与真实游戏记录。

## 直接启动

项目根目录执行：

```bash
uv run --project src/presentation --offline python src/presentation/server.py
```

打开 **http://127.0.0.1:8765**。远程开发盒可将 8765 端口转发到本机。默认查找 `build/prim-rs/prim`、`target/release/prim`、`target/debug/prim`；使用 `--runtime /absolute/path/to/prim` 覆盖。服务不会自动编译 Prim 或申请 worker。

参数还包括 `--host`、`--port`、`--outline`、`--presenter`、`--course`、`--date`、`--no-runtime`。文案变更后刷新网页即可；前端模块改动后需要重新构建。构建资源和 Python 环境已在本工作区准备好，首次部署请执行下一节。

## 首次部署与前端构建

Python 由 uv 管理，前端依赖由 npm lockfile 固定，无 CDN 运行依赖：

```bash
labpon
uv sync --project src/presentation --locked --group export
npm ci --prefix src/presentation --ignore-scripts
# Prim 扩展尚未安装依赖时：
npm ci --prefix src/prim-language-server --omit=dev --ignore-scripts
```

前端构建涉及 Monaco 多模块打包。在 PJLab 环境按全局规则使用有效 CPU worker：先以 `list-worker.py` 和 `--ssh` 发现当前 worker，再通过脚本输出的 SSH 执行以下命令；项目路径使用 `realpath` 后的绝对路径。

```bash
cd /absolute/shared/path/to/Prim
npm run build --prefix src/presentation
```

不把历史 worker 地址写入配置。无可用 worker 时停止重构建，不自行申请 pod。构建产物位于 `build/presentation-web/`，server 从该目录提供前端模块。其缺失时页面会明确提示先构建。

## 页面与现场运行

| 功能 | 操作 |
| --- | --- |
| 翻页 / 分步 | ← / →、空格、PageUp / PageDown、底部按钮；有分步内容时先逐步展开再翻页 |
| 代码页内翻页 | PageUp / PageDown（翻页笔）始终有效；方向键留给编辑器与游戏 |
| 目录 / 全屏 | O / F，或顶部对应按钮 |
| 首尾页 | Home / End |
| 场景跳转 | URL 片段如 `#14`；编辑或弹窗内不会误触翻页 |
| 运行 | 现场运行页的“运行”或 Ctrl+Enter / Cmd+Enter；“解释 / JIT / 检查”切换执行方式 |
| 结束进程 | “停止”；离开该页时也会停止当前交互会话 |
| 恢复代码 | “重置本例”；“回到演示代码”从 builtin 定义返回 |
| 订单报告 | orders.pr 运行完成后可下载本次报告 |
| 游戏 | 方向键/WASD 移动，P 暂停，R 重启，Q 退出 |

代码草稿按场景保存在当前浏览器。在线实验台不把预期输出当实际结果，stdout、诊断、编译与入口统计都来自真实程序。短例子使用独立进程；要观察 JIT 入口缓存，必须在同一程序内重复调用。贪吃蛇使用已有 `src/prim-rs/examples/snake.pr`，浏览器中没有另一份游戏规则。

## 编辑器与 Prim 扩展

编辑器使用 Monaco，直接读取现有扩展的 `package.json`、语言配置和 TextMate grammar，通过 `vscode-textmate` / `vscode-oniguruma` 完成高亮。服务器启动现有 `src/prim-language-server/server/server.js --stdio`，以 WebSocket 桥接 LSP，提供补全、诊断、悬停、F12 定义、引用、F2 重命名、签名、语义高亮、折叠和 inlay hints。

这不是任意 VSIX 的通用插件宿主。复用的是当前 Prim 扩展的声明与真实语言服务，不复制一套静态分析器，也不声称桌面扩展的 Node/IPC 入口能直接在浏览器运行。builtin `.pri` 定义可在编辑器只读打开；接口文件不作为可执行代码提交。

视觉采用 Claude 官网的配色（象牙底、石墨字、陶土色强调）。字体全部随应用本地加载：标题 Source Serif 4 + Noto Serif SC（按 GB2312 一级字与文案子集化），正文 Inter，代码与终端 JetBrains Mono；许可证见 `static/fonts/LICENSE.txt`。文案新增生僻字时，按该文件记录的方式重新子集化 Noto Serif SC。编辑器与终端是同一组实例，进入现场运行页时移入该页，字号随幻灯片宽度缩放。

## 执行架构与边界

同一个 aiohttp server 提供：

- HTTP：文案、静态资源、短代码执行、实测记录与复测接口。
- `/ws/lsp`：JSON-RPC 与 Node LSP 的 Content-Length stdio 消息互转。
- `/ws/terminal`：浏览器与独立 PTY 进程的双向连接，包含按键、resize、停止和输出确认。

服务默认回环监听，执行与 WebSocket 请求检查同源及会话 token。代码通过临时文件传递，不拼接 shell 命令。短例子限制为 32 KiB 源码、64 KiB 输出、6 秒墙钟时间和运行步骤预算。交互代码最多 64 KiB，PTY 会话最多 180 秒和 16 MiB 总输出；还有 CPU、地址空间及文件大小上限。输出使用有界队列与客户端 ACK 提供背压。最多同时打开四个 LSP 和两个 PTY 会话，断开会回收子进程。

Prim 支持文件和环境接口。这里是可信课堂演示工具，资源限制不是文件系统或网络沙箱；不要直接作为公开的匿名执行服务使用。矩阵与机器码场景不加 `--fuel`，因为它会关闭 JIT 的局部标量路径；这两个 PTY 会话仍受时间、输出与资源上限约束。

## 测量与数据处理

`scenarios/jit.pr` 是 80×80 整数矩阵乘法 C = A × B 再加权求和，A、B 元素按公式即时生成（每次调用 51.2 万次乘加）。内核只用局部整数，满足 JIT 局部标量路径的条件；结果与 `native.py` 中独立的 Python 参考逐项核对。

`scenarios/orders.csv` 是固定种子 20261019 生成的 600 条合成订单，字段为 category、quantity、unit_price_cents、valid，金额使用整数分。

`orders.pr` 用数据闭包保存四列数据，逐字符解析、过滤、按类别聚合并写出报告。测量脚本独立计算行数、有效订单、总金额、八个类别金额和重复调用 checksum，逐项核对解释与 JIT 两种模式。

在有效 CPU worker 中运行正式测量：

```bash
uv run --project src/presentation --offline python src/presentation/benchmark.py
```

每种模式进行 3 次独立进程测量，每进程内部重复调用 5 次，取中位数。记录首次调用、批次均摊热调用、内部端到端与完整进程时间；Prim 内部时钟精度是毫秒。小于分辨率的值不用于夸大加速比。

结果保存为 `build/presentation/evidence/benchmarks.json`，包含真实 stdout/stderr、命令、机器、LLVM 版本、执行程序和输入摘要。测量前后摘要变化会拒绝发布。网页核对源码和当前二进制是否与记录匹配，历史记录不会冒充当前版本测量。

实战页的“重新测量”运行每模式单样本的快速版本，会明确更新样本数；不要在运行游戏的同时复测。需要用于正式结论时重新执行多样本命令。订单场景 JIT 收益有限，也如实保留原始结果。

## 浏览器验证与游戏截图

```bash
uv run --project src/presentation --offline python -m unittest discover -s src/presentation/tests -v
uv run --project src/presentation --group export --offline python src/presentation/check_browser.py
```

浏览器检查验证分步翻页、`std.inspect` 页面、真实 LSP、各现场运行页、矩阵与订单 PTY、贪吃蛇按键和多种视口，临时 server 在完成后关闭。游戏在实际暂停状态的截图保存为 `build/presentation/evidence/snake.png`，对应来源信息保存在 `snake.json`。这是运行记录，不是浏览器模拟画面。测试不代替 Prim 全项目的完整验收。

## 导出 PPTX

首次准备浏览器：

```bash
labpon
export PLAYWRIGHT_BROWSERS_PATH="$(pwd -P)/build/presentation-tools/browsers"
uv run --project src/presentation --group export --offline playwright install chromium
```

建议先完成测量与浏览器检查，确保三个实战页都有实际证据，再导出：

```bash
uv run --project src/presentation --group export --offline python src/presentation/export.py
```

默认导出全部页面，`--main-only` 仅导出主讲页。`--output` 指定产物；`--presenter`、`--course`、`--date` 指定封面元信息；`--url http://127.0.0.1:8765` 可导出现有服务。

输出为 `build/presentation/prim-class-presentation.pptx`，以及对应 `-assets/` 下的逐页 PNG、总览图和 manifest。PPT 页面是网页截图，演讲者备注仍可编辑；离线 PPT 无法运行代码。导出不重新执行程序，而是呈现已标明来源的测量及游戏记录。

共享挂载若不支持 Chromium 的 mmap，导出器会临时复制浏览器到本机临时目录，结束后清理，需约 500 MiB 可用空间。

## 文件分工

| 文件 | 职责 |
| --- | --- |
| `content.py` | 解析 Markdown：章节、版式、分步标记、代码块与页码 |
| `server.py`、`channels.py` | HTTP 服务、LSP 与 PTY WebSocket |
| `runner.py`、`limits.py`、`terminal_child.py` | 执行、资源限制及会话生命周期 |
| `web/editor.js`、`web/lsp.js` | Monaco、现有扩展适配、xterm.js |
| `static/` | 展示布局、图示、导航、字体与现场运行页 |
| `scenarios/`、`benchmark.py`、`native.py` | 矩阵与订单程序、固定输入、独立参考、真实测量与机器码导出 |
| `demos.json` | 场景、源码来源和预期说明 |
| `export.py` | 网页截图与 PPTX 构造 |

课堂文案可按实际实现更新；本工具不修改语言规范、Prim 运行时或现有语言服务器源码。
