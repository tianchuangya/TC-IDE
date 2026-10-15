# TC-IDE 进度交接（给下一个 AI）

> 生成时间：第 21 轮结束时。工作目录 `/home/tianchuangya/桌面/TC-IDE`，分支 `main`。
> 本文件描述**当前真实状态**：做了什么、怎么验证、还差什么、以及**已知的失败**。
> 与 `docs/HANDOFF.md`（原始需求队列，权威）配合阅读：本文件是"进度快照 + 接手说明"，HANDOFF 是"任务清单"。

---

## 0. 一分钟摘要

- 项目：**TC-IDE**，原生 Rust 多平台 IDE，**零第三方运行时依赖**（Linux 手写 X11/Cairo/Pango FFI，Windows 用 `windows` crate 但仅 cfg 内，macOS 手写 Objective-C 运行时 FFI，没有 WebView / 没有 HTML）。
- 三个后端：`linux_app.rs`（成熟）、`windows_app.rs` + `windows_app/`（成熟，14 个模块）、`macos_app.rs`（**最近 7 轮从 5 行占位发展到 4585 行可用后端**）。
- 共享纯逻辑内核（跨平台、在 Linux 上就能跑单元测试）：`editor_core`、`editor_session`、`graph_core`、`terminal_core`、`text_buffer`、`session_core`、`workspace_core`、`ai_core`、`extension_core`、`lsp_core`、`dap_core`、`syntax_core`、`index_core`、`motion_core`、`package_motion`、`background_core`、`theme_core`，共 **17 个**，全部在 `main.rs` 声明，并被 `tests/platform_boundary.rs` 守着（禁止平台类型/`unsafe` 泄漏进共享核心）。
- 本地测试：**235 项共享内核/平台后端测试 + 4 项平台边界测试 + 3 项模块结构测试**（`cargo test --all-targets`），另有只在各平台 CI 里跑的 Windows/macOS 专属测试。
- 任务队列（`docs/HANDOFF.md` 第 5 节）：**40 项已勾选 / 3 项部分完成 / 0 项未开始**。
- 当前 CI 已恢复全绿：`ba427f2` 的 format / Linux / Windows typecheck / Windows / macOS 五个 job 全部通过。

---

## 1. 仓库结构

```
src/
  main.rs                入口 + 模块声明（17 个共享核心都在这里声明）
  linux_app.rs           Linux 后端（X11 + Cairo + Pango，手写 FFI）
  windows_app.rs          Windows 后端入口（布局守卫限制 ≤1500 行）
  windows_app/*.rs        Windows 子模块（14 个，每个 ≤1000 行）
  macos_app.rs           macOS 后端（Cocoa，Objective-C 运行时 FFI，4585 行）
  windows_platform.rs / windows_preferences.rs / windows_renderer.rs / windows_terminal.rs
  <17 个共享核心>.rs
tests/
  platform_boundary.rs   守住共享核心：不许出现平台导入、窗口类型、unsafe
  module_layout.rs       守住模块结构：模块注释、行数上限、渲染区域必须有画笔
tools/
  windows_editor_sim.py  在 Linux 上提取并跑 Windows 编辑器方法的模拟器
.github/workflows/build.yml   CI：format / linux / windows-typecheck / windows / macos
docs/
  HANDOFF.md             原始任务队列（权威清单，39/4/0）
  PROGRESS-TRANSFER.md   本文件
  ARCHITECTURE.md / INSTALL.md / KNOWN-ISSUES.md / REAL-MACHINE-CHECKLIST.md
```

---

## 2. 最近各轮做了什么（按提交，新→旧）

### 第 21 轮

- Linux 新建项目统一走共享模板：C++ 是 `main.cpp`，不再生成开头说明注释。
- Linux 接入共享 DAP 会话、断点、继续/停止/单步，停止后抓取栈/作用域/变量；源码上方绘制带连线的变量值标签和当前执行流。
- 新增“查看”菜单；调用层级和终端可独立隐藏，分隔线可拖动，开关/栏位使用原生弹簧过渡。
- 三平台语法颜色增加类型、函数、常量、预处理指令，深浅主题均通过对比度测试；CI 新增 debug/view 原生帧烟雾测试。
- 本地 Linux、Windows MSVC 目标、macOS 目标的 `check` 与 `clippy -D warnings` 均通过；仍需等待本轮推送后的 GitHub Actions 结果。

### 第 20 轮

| 提交 | 内容 |
| --- | --- |
| `ba427f2` | 修正 macOS 工具链自检中写反的部分安装断言：只安装 `rustfmt` 时，摘要应报告缺失的 `rust-analyzer`，而不是列出已经安装的格式化器。五个 CI job 全绿。 |
| `e4ff947` | macOS 的真实工具探针改用确定的绝对路径，避免 runner `PATH` 差异；同时修复 AI SSE 集成测试的 Python 脚本，并让测试服务器启动失败时明确失败，不再静默跳过。 |

### 第 19 轮（本轮）

| 提交 | 内容 |
| --- | --- |
| `aa82388` | **macOS 扩展与工具链面板**：`⌘E` 打开，用共享 `extension_core::language_tools` + `lsp_core::executable_available` 真机探测 7 种语言的 LSP/格式化/调试（✓/· 三列 + 工具名），回车重探并在状态栏列出缺失项；`⌘⇧F` 用共享 `workspace_core::format_command` 把格式化命令送进终端执行。**CI 失败（见 §5）** |
| `c5b917b` | **macOS 代码着色**：代码列按共享 `editor_core::syntax_tokens` 逐 token 绘制（关键字/字面量/注释/正文各取共享调色板颜色），横向滚动按列切片（切片为纯函数 `visible_token`，有 36 组不变量的测试）。CI 绿 |
| `0b265e9` | **macOS 设置面板**：`⌘,` 打开；控件状态走共享 `theme_core::control()`（正常/悬停/按下/禁用 + 焦点环）；`Tab`/`⇧Tab` 只走**可用**控件、`Enter`/空格使用、`Esc` 关闭；深/浅主题、编辑器字号步进、界面语言；偏好经共享 `workspace_core` 读写到 `~/Library/Application Support/TC-IDE/settings.conf`，启动时读取。CI 绿 |
| `ec377eb` | **macOS 关系图面板**：右侧面板由共享 `graph_core` 驱动（打开的文件即索引，光标所在符号即焦点），范围/关系/折叠/选择全用共享模型，几何沿用 `graph_core` 的 `ROW_HEIGHT`/`FIRST_ROW_TOP`/`visible_rows`；`⌘G` 开关、`⌘⇧G` 切范围、上下键选择、空格折叠、`Tab` 切关系、回车跳转、点击选中、`Esc` 交回键盘。CI 绿 |

### 第 18 轮

| 提交 | 内容 |
| --- | --- |
| `f05b756` | **macOS 真实终端（PTY）**：`forkpty` 起登录 shell，后台线程读主设备，经新增的共享 `terminal_core::Utf8Chunker`（跨读边界不拆多字节字符）交给共享 `TerminalParser`；底部抽屉用 `terminal_core::panel_metrics` 布局 + 共享弹簧动画；`⌘J` 开关、`PageUp/Down` 翻历史；键盘聚焦时整键转发（固定序列来自新增共享 `terminal_core::key_sequence`，可打印字符走 `insertText:` 以配合 IME）；尺寸变化用 `ioctl(TIOCSWINSZ)` 同步；退出前先 `close()` shell。**CI 在 macOS runner 上真起了 shell 并读回输出**。CI 绿 |

### 第 17 轮

| 提交 | 内容 |
| --- | --- |
| `3fa7c36` | **macOS 标签栏**：标题栏画出所有打开的文档（活动标签高亮 + 脏标记），点击切换、`⌘⇧]`/`⌘⇧[` 循环、`⌘W` 关闭（拒绝关最后一个）；命中测试与绘制共用 `tab_index_at`/`TAB_WIDTH`。CI 绿 |
| `064a1b7` | **macOS 剪贴板 / 打开面板 / 访达 / 菜单栏 / 实测字宽**：`NSPasteboard` 复制剪切粘贴、`NSOpenPanel` 打开文件（新标签）与文件夹、`NSWorkspace` 在访达显示、`NSMenu` 应用/文件/编辑菜单（菜单项与快捷键共用 `command_for_menu_action` 一张表）、字宽用 AppKit `sizeWithAttributes:` 实测并按字号缓存。配套把选区文本/剪切/粘贴/撤销重做（**按编辑分组**，修掉了"撤销只掉一个字母"的真实缺陷）/按文件打开新标签下沉到共享 `editor_session`。CI 绿 |

### 第 16 轮

| 提交 | 内容 |
| --- | --- |
| `bbce6e4` | 把 macOS 自检里的断言抽成共享 `editor_session::state_check`，Linux 单元测试用同样数字调用 → 两边不会漂移。**这一步立刻抓出两个真实缺陷**：点到只有 2 列的行上第 3 列；标签脏标记断言的对象搞错。CI 绿 |
| `48cb589` | **新增共享核心 `editor_session`**：打开的文档与标签、光标与选区、输入（含覆盖选区）、退格/Delete 的边界与并线、方向键与 Home/End（列号夹取）、全选、保存、视口跟随、按行/列滚动、`line_at`/`column_at` 点击映射。macOS 后端改为薄壳。`TextBuffer` 增加 `ends_with_newline`。CI 绿 |
| `6f04ed6` / `34fd0a9` | 两次 macOS 自检修复（点击落在行长之外、自检用已知文档） |

### 更早（第 14 轮及以前，摘要）

- Linux 后端：设置面板 14 个控件（悬停/按下/禁用/焦点环、Tab 走查）、背景图裁剪预览与拖拽、8 个渲染场景（`--render-scene`）供 CI 做帧门槛。
- Windows 后端：拆成 14 个子模块（行数/结构守卫）、共享断点表与条件断点、格式化选区与标记等待、打包浮层使用共享主题控件状态。
- 共享核心：`theme_core`（调色板 + 对比度规则 + 控件状态，浅色主题专门的 token 配色）、`syntax_core`（接收者/跨文件解析/声明表）、`graph_core`（声明与调用解析、边类型、折叠）、`dap_core`（断点规格）、`workspace_core`（格式化命令/打包命令/偏好读写）、`extension_core`（版本比较与更新计划）、`motion_core`（行弹簧与编辑脉冲）、`package_motion`、`background_core`、`index_core`。

---

## 3. macOS 后端现状（重点，因为变化最大）

`src/macos_app.rs`（4585 行）**完全手写 Objective-C 运行时 FFI，不引入任何绑定 crate**：

- **已实现**：`NSApplication`/`NSWindow` + 自注册 `NSView` 子类 `TcIdeEditorView`（`drawRect:`/`keyDown:`/`insertText:`/`mouseDown:`/`mouseMoved:`/`mouseUp:`/`isFlipped`/`acceptsFirstResponder`）、60Hz `NSTimer` 驱动共享动效与重绘、标题栏/标签栏/行号栏/代码列（**逐 token 着色**）/选区/呼吸光标/状态栏、真实 PTY 终端抽屉、关系图面板、设置面板、扩展与工具链面板、剪贴板、打开面板、访达显示、菜单栏。
- **编辑能力全部来自共享 `editor_session`**（不含平台代码），macOS 侧只留键位映射、窗口几何与绘制。
- **字宽实测**：`measure_cell` 用 `sizeWithAttributes:` 量 10 个数字取平均，按字号缓存；失败才回落到常量 `TEXT_CELL = 7.4`。光标/选区/点击映射都用它。
- **状态层自检**：`./target/release/tc-ide --self-check`，在 macOS runner 上真的执行（起 PTY、读输出、建关系图、探工具），并把共享 `editor_session::state_check` 的断言一起跑。
- **仍未做**：Metal 渲染、直接 CoreText 排版（目前用 AppKit 文本绘制 API + 实测字宽）、打包面板（macOS 没有对应打包流程；Windows 的 ZIP/MSIX 命令是 PowerShell，不能直接用）。

---

## 4. 任务队列状态（`docs/HANDOFF.md` 第 5 节）

**40 已勾选 / 3 部分完成 / 0 未开始**。仍以 `docs/HANDOFF.md` 中的逐项说明为准。

| 编号（文件行号） | 主题 | 已做 | 还差 |
| --- | --- | --- | --- |
| 165 | 语义关系 | `syntax_core` 语言感知操作、接收者（`SelfRef`/`Type`/`Value`）、跨文件解析、构造绑定、`DeclarationRef`、置信度 | **一般表达式与返回类型分析**（仍不建真正的语法树） |
| 203 | 控件状态 | 共享控件状态模型 + Linux 完成（悬停/按下/禁用/焦点环）；macOS 设置面板也用上了 | Windows 设置/扩展浮层仍是"只有悬停" |
| 208 | macOS 原生后端 | 窗口、绘制、着色、编辑、标签、剪贴板、打开面板、访达、菜单、实测字宽、**终端**、关系图、设置、扩展工具链 | Metal 渲染、直接 CoreText 排版、打包面板；**以及真机观感确认** |

> Windows 实机验收（键盘、DPI、IMM32、ConPTY、ZIP/MSIX 打包）无法在本机完成，已交付：
> `docs/REAL-MACHINE-CHECKLIST.md`（可执行清单）+ `tc-ide.exe --self-check`（状态层自检）。

---

## 5. CI 状态

`ba427f2` 对应的第 153 次构建已全绿。此前 macOS `--self-check` 的失败已确认并修复：部分安装探针模拟“只有 `rustfmt`”时，摘要理应列出缺失的 `rust-analyzer`，旧断言却要求出现已经安装的 `rustfmt`。同时，真实可执行文件探测已改为不依赖 runner `PATH` 的绝对路径。

排查这类平台专属失败时，优先打开已登录的 GitHub Actions 页面读取自检打印的 `FAILED: ...`；公共 API 能给出 job/step 状态，但未认证的日志下载可能返回 403 或很快触发速率限制。

---

## 6. 本地验证配方（每次改动都要跑）

```bash
cd /home/tianchuangya/桌面/TC-IDE

# 1. 格式化（必须用 stable 工具链：宿主 /usr/bin/rustfmt 1.7 与 CI 的 1.10 不一致）
~/.cargo/bin/rustup run stable cargo fmt --check

# 2. 全部测试（233 + 4 + 3）
~/.cargo/bin/rustup run stable cargo test --all-targets

# 3. 三个目标的 lint（零警告门槛）
~/.cargo/bin/rustup run stable cargo clippy --all-targets -- -D warnings
~/.cargo/bin/rustup run stable cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings
~/.cargo/bin/rustup run stable cargo clippy --all-targets --target x86_64-apple-darwin -- -D warnings

# 4. 跨目标类型检查（Darwin 目标本机已安装，只做类型检查不链接）
~/.cargo/bin/rustup run stable cargo check --all-targets --target x86_64-apple-darwin

# 5. Windows 编辑器逻辑模拟（在 Linux 上跑 Windows 编辑路径）
python3 tools/windows_editor_sim.py

# 6. Linux 渲染帧（无需映射窗口）与帧对比
DISPLAY=:0 cargo run -- --render-frame /tmp/frame.png
DISPLAY=:0 cargo run -- --render-frame /tmp/settings.png --render-scene settings-hover
```

> 注意：**默认 `cargo`（1.75）没有 darwin target**，跨目标命令必须走 `~/.cargo/bin/rustup run stable cargo`。

CI（`.github/workflows/build.yml`）五个 job：`format`、`linux`（测试 + Xvfb 渲染帧门槛）、`windows-typecheck`（Windows **和** Darwin 的 check+clippy）、`windows`（测试 + release 构建 + `--self-check`）、`macos`（release 构建 + `--self-check` + `cargo test --all-targets`）。

---

## 7. 硬约束（不要违反）

- ❌ 不创建版本标签、安装包或 GitHub Release；`release.yml` 只允许手动触发，**绝不触发**。
- ❌ 不引入 HTML/CSS/WebView 运行时。
- ❌ 不在仓库、配置、日志或命令行里写任何密钥/API 密钥（AI 密钥只用环境变量 `TC_IDE_AI_KEY` 传给子进程）。
- ❌ 不用破坏性 git 命令；保留用户改动。
- ✅ 提交信息用 heredoc 文件 + `git commit -F`（不要用双引号字符串内联反引号）。
- ✅ 每个改动都要：补共享纯函数 + 单元测试 → 本地跑 §6 全部命令 → push `main` → 等 CI 全绿。

---

## 8. 建议的下一步（按性价比排序）

1. **补齐 Windows 控件状态**：设置/扩展浮层的按下、禁用、键盘焦点状态。
2. **语义分析增量（165）**：给 `syntax_core` 加最小可用的一般表达式/返回类型推断（例如记录 `fn` 的返回类型，让 `let x = self.build(); x.prepare()` 能解析），并保持全部在共享核心里、有单元测试。
3. **macOS 打包/构建面板**。
4. **macOS Metal/CoreText**：工作量最大、本机最不可验证（本机只能类型检查 + 靠 CI 的 macOS runner 跑自检），建议放在最后，且先做"能跑起来、有自检断言"的最小版本。

---

## 9. 给下一个 AI 的工作方式建议

- **每轮只做一件事**，做完立刻验证 + 提交 + 等 CI，别攒着。CI 一轮约 4–6 分钟。
- **把能在 Linux 上跑的断言尽量下沉到共享核心**（第 16 轮的 `state_check` 就是范例）：CI 的 macOS/Windows 日志读不到，只有本地可跑的断言才可调试。
- **改 `macos_app.rs` 时小心 Python 批量替换**：本会话多次因 `cargo fmt` 重排导致锚点失配（`AssertionError`/`StopIteration`），且有一次**整个文件被旧草稿覆盖、把之前的修复回滚了**（第 15→16 轮）。改完一定 `git diff --stat` 复核，并重跑全部验证。
- 用 `git log --oneline` + `docs/HANDOFF.md` 的 `[x]/[~]/[ ]` 标记作为唯一进度真相，不要相信对话记忆。
