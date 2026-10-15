# TC-IDE 项目交接与后续任务

> 最后更新：2026-10-15（第二批：共享内核、会话、AI/扩展、打包、LSP 协议层与文档）  
> 仓库：<https://github.com/tianchuangya/TC-IDE>  
> 当前分支：`main`  
> 交接基线：`a87722c` 及其后续提交（终端批次与关系图批次见第 5 节勾选状态）  
> 当前阶段：Windows 原生后端功能对齐中，**尚未达到正式发布条件**

## 1. 必须遵守的项目约束

1. **未经项目所有者明确说“正式开始”，禁止创建版本标签、安装包 Release 或 GitHub Release。**
2. 日常工作只允许修改源码、测试、文档，提交并推送到 `main`；`.github/workflows/release.yml` 只能手动触发。
3. TC-IDE 不是网页套壳。产品运行时禁止使用 HTML/CSS/WebView；Linux、Windows、macOS 分别使用原生窗口和原生绘制接口。
4. 用户要求采用队列式推进：旧任务未完成时，不因新需求而遗忘旧任务。新需求应加入本文待办队列，再按优先级继续。
5. 不应宣称“Windows 已全部可用”，直到完成 Windows 实机验收。GitHub Actions 目前主要证明编译、链接和单元测试通过，不等于全部 GUI 行为已在真机验证。
6. 不得把 Linux 可执行文件改名伪装成 Windows/macOS 程序，也不得声称 Windows 能直接原生生成 `.deb`/`.dmg`。跨系统产物必须由对应系统 runner 或明确的交叉构建工具生成。
7. API Key、密码和其他秘密不得写入仓库、配置文件、日志或提交历史。AI API Key 目前设计为仅保留在进程内存中。
8. 修改已有功能时保留用户改动，不使用破坏性的 Git 命令，不创建无关 Release。

## 2. 产品目标与视觉方向

TC-IDE 是 Rust 原生、低色彩噪声、强调空间关系与运动品质的 IDE。目标不是堆叠霓虹色，而是：

- 深色和浅色两套克制的语义主题；
- 类似 Apple 界面的清晰层级、圆角、阻尼弹簧和连续过渡；
- 代码输入/删除、面板、树节点、终端、关系路径都具有轻量且不中断交互的动画；
- 关系图优先保证可读性，按 `项目 → 文件/路径 → Class → 函数/变量` 表达；
- 悬停代码符号时，符号本身出现边框，并从代码位置沿轨道连接同文件调用；跨文件关系指向左侧路径树；
- 注释气泡位于代码右侧、限制最大宽度、支持多行、折叠、展开和受保护编辑；
- 打包动效使用手绘抽象线条黑洞/白洞：顺时针吸入、反向和换色吐出，代码元素越接近中心速度越快并发生拉伸扭曲；
- 所有界面随 DPI 和窗口尺寸缩放，不使用固定小窗口假装成完整 IDE。

## 3. 当前仓库结构

```text
TC-IDE/
├── Cargo.toml                 # Rust 依赖；Windows 依赖按 cfg(windows) 隔离
├── Cargo.lock
├── build.rs                   # 构建脚本
├── README.md                  # 用户入口说明；部分路线描述可能滞后，以本文和代码为准
├── TC-IDE.desktop             # Linux 桌面入口
├── assets/
│   ├── tc-ide-icon.png
│   └── fonts/JetBrainsMono-Regular.ttf
├── docs/
│   ├── ARCHITECTURE.md        # 初始架构草案，部分能力表已滞后
│   └── HANDOFF.md             # 本交接文档
├── src/
│   ├── main.rs                # 按目标系统选择平台入口
│   ├── ai_core.rs             # 共享 AI 请求体、端点校验、响应/流式/状态码解析与命令
│   ├── extension_core.rs      # 共享扩展目录、下载/校验/解包/卸载命令与运行时缓存清单
│   ├── lsp_core.rs            # 共享 LSP 分帧、请求构造与诊断/补全/悬停/位置/重命名解析
│   ├── session_core.rs        # 共享文档会话、脏状态、最近项目与会话恢复文件
│   ├── workspace_core.rs      # 共享偏好设置、项目模板、任务/格式化/调试命令与打包报告
│   ├── editor_core.rs         # 共享语言识别、注释、语法 token、补全、符号关系与声明分类
│   ├── graph_core.rs          # 共享关系图范围、搜索、层级、视口裁剪和节点预算
│   ├── terminal_core.rs       # 共享终端网格换算、滚动历史、选区、文本提取与 VT 解析器
│   ├── linux_app.rs           # X11/Cairo/Pango Linux 实现，文件较大
│   ├── windows_app.rs         # Win32 状态、事件、编辑、项目、终端和流程编排
│   ├── windows_platform.rs    # Windows 文件对话框、菜单、剪贴板、Shell/DPI API
│   ├── windows_preferences.rs # Windows 持久化设置
│   ├── windows_renderer.rs    # DirectWrite/Direct2D 场景绘制
│   ├── windows_terminal.rs    # ConPTY 与 VT 输出解析
│   └── macos_app.rs           # macOS 隔离入口，功能仍明显落后
└── .github/workflows/
    ├── build.yml              # push/PR：格式检查、Linux 编译测试、Windows 类型检查、三平台编译测试
    └── release.yml            # 仅手动发布；未获授权不得触发
```

### 当前分层现实

`editor_core.rs`、`graph_core.rs`、`terminal_core.rs` 已经是共享纯 Rust 核心，且都在三个平台上参与编译、由 Linux 上的 `cargo test` 覆盖；但 Linux 和 Windows 仍各自保存大量 UI、项目与交互逻辑。`windows_app.rs`、`windows_renderer.rs`、`linux_app.rs` 都偏大，后续需要逐步拆分；不要为了“重构漂亮”一次性重写整个后端，应在测试保护下按模块迁移。

Windows 特有逻辑的本地验证方式已补齐：`cargo check --all-targets --target x86_64-pc-windows-msvc`（需要 `rustup target add x86_64-pc-windows-msvc`）可以在 Linux 上对 `cfg(windows)` 代码做不链接的类型检查，CI 的 `windows-typecheck` job 也会执行同一命令。它只能证明类型与 API 正确，不能替代 Windows 实机验收。

## 4. 已完成并经过 CI 的能力

最新相关功能在 Windows、Linux、macOS GitHub Actions 中均通过编译/测试；最后一次确认的功能提交为 `a87722c`。

### 4.1 共享编辑器核心

- 按后缀识别 C、C++、Rust、Python、Java、Kotlin、TypeScript/JavaScript、Go、Web、C#、Swift、Zig、Lua、Ruby、PHP、R、SQL、Shell、JSON、TOML、YAML、Markdown 等；未知类型回退 Plain Text。
- C/C++ 的 `#include` 不会误判为注释。
- 语言相关关键字补全、基础语法 token、项目符号候选。
- 声明/调用角色与跨文件项目符号索引。
- 当前本地测试为 22 项。

### 4.2 Windows 原生编辑器与系统集成

- Win32 原生窗口，Per-monitor DPI；DirectWrite/Direct2D 渲染。
- 文件/文件夹选择、Explorer 打开目录、UTF-16 系统剪贴板、中文 IME。
- 鼠标拖选、Shift 扩选、方向键、Home/End、跨行光标、Unicode 编辑、撤销/重做。
- `Ctrl+A/C/X/V/S`、字号快捷键、保存和打开。
- 深浅主题、中英文 UI、背景图片、透明度、位置和 1.0–3.0 倍缩放。
- 字体搜索和真实 DirectWrite 字体切换；选择会持久化。
- 设置、扩展、打包和新建浮层使用原生阻尼弹簧位移动画。
- 项目区空白处原生右键菜单：新建项目、文件、文件夹和刷新。

### 4.3 项目、语言和环境

- 递归项目树、目录折叠、搜索、滚轮和文件打开。
- 新建项目/文件/文件夹弹窗具有可点击的确定/取消按钮、系统路径选择和同名错误提示。
- 项目语言与环境可以选择 `None`。
- Python 支持 `venv`、`uv`、`conda`、`poetry` 初始化命令；Rust stable/nightly、Go modules 和 Web 包管理器具有基础初始化路径。
- 新项目会显示真实版本，例如 Python 3.13/3.12/3.11，而非 `v1/v2/v3`。
- 运行时缓存目前是版本元数据目录，不是完整的自动下载运行时仓库。

### 4.4 注释与关系图

- 注释气泡按当前文件语言解析，位于编辑器右侧，鼠标进入项目树时隐藏。
- 支持展开、折叠、编辑；退格、删除、粘贴和选择不会越过注释正文边界。
- 悬停函数/Class/标识符时出现边框和从代码位置生长的路径。
- 右侧关系图显示项目、文件、声明/调用位置和真实相对路径。
- 图节点支持滚轮分页和点击跨文件跳转。
- 切换项目会刷新符号缓存，避免显示旧项目关系。
- 关系图由共享 `graph_core` 驱动（Windows 已接入）：`auto_scope` 决定范围（显式选择 > 搜索 > 悬停符号 > 当前文件），支持按符号名或路径的范围内搜索、`项目 → 文件夹 → 文件 → Class → 函数/变量` 缩进层级、`declare/call` 角色区分、视口裁剪与 `MAX_NODES` 预算；点击卡片按节点自带目标跳转，不再用行号反推符号。
- `editor_core` 现在还会识别 `let/const/var/static` 变量声明并按行分类声明种类（Class/Function/Variable），同时排除 `if let ...` 一类流程语句，变量因此能出现在关系图里。

### 4.5 扩展中心

- 分类为 Languages、Themes、Debuggers、Formatters、AI / Agents。
- Windows Languages 已扩展为 16 个常用入口，支持搜索、滚轮分页、版本和安装状态持久化。
- Themes 可真正切换深色/浅色。
- Debuggers/Formatters 点击后会在 Output 终端真实探测 LLDB、GDB、debugpy、Biome、clang-format、Ruff 是否存在及其版本。
- AI 页面可编辑 OpenAI-compatible Endpoint、模型和会话级 API Key；Endpoint/模型持久化，Key 不落盘。

### 4.5.1 共享内核与测试（2026-10-15 第二批）

- `terminal_core`：网格、滚动、选区、VT 解析器、Windows 环境块；`graph_core`：范围/搜索/层级/折叠/边类型/视口；
  `session_core`：文档会话、脏状态、最近项目、恢复文件；`workspace_core`：偏好设置（兼容两套键名）、
  项目模板、任务/格式化/调试命令、打包目标与报告；`ai_core`/`extension_core`：AI 请求与响应、扩展目录与运行时缓存；
  `lsp_core`：LSP 分帧、请求与响应解析。
- 本地测试数量从 22 项增加到 115 项，全部在 Linux 上运行；Windows 侧新增 `tc-ide --self-check` 状态层自检。

### 4.6 Windows 终端、运行与打包

- 多个真实 ConPTY 会话；Build、Run、Output 和用户新增终端。
- 终端输入输出、VT/CSI/OSC 基础解析、方向键、Home/End/Delete/Tab；解析器已下沉到共享 `terminal_core`（`TerminalParser`/`TerminalView`），在 Linux 上也有 7 项单元测试覆盖 OSC 跳过、CSI 光标与擦除、退格改写、清屏、800 行滚动上限和视图发布。
- `Ctrl+C/V/A/L` 交给终端；`Ctrl+Tab`/`Ctrl+Shift+Tab` 切换终端。
- 终端网格由窗口尺寸换算：底部面板使用共享的 `terminal_core::panel_metrics` 几何与 `GridSize`，弹出窗口用 GDI 实测等宽字体字宽/行高；`WM_TIMER`/`WM_SIZE` 上的签名比较保证窗口缩放、DPI 变化、字号变化和新增会话都会同步 `ConPtySession::resize`。
- 底部面板保留滚动历史：滚轮在面板正文上翻看输出、输入时自动回到底部，标题显示偏移行数；面板正文支持鼠标拖选、按等宽单元格高亮显示，状态栏报告选中字符数，`Ctrl+Shift+C` 复制（`Ctrl+C` 仍是中断）。
- 弹出终端窗口拥有独立滚动偏移、鼠标拖选（按等宽单元格高亮）、`Ctrl+Shift+C` 复制（`Ctrl+C` 仍是中断）、中文 IME 组合，并在弹窗存在期间接管所显示会话的网格；关闭弹窗后 IDE 会重新按面板尺寸同步该会话。
- Build/Run 根据 Cargo、npm、Go、Python、C、C++、Java 项目选择命令。
- 活动 ConPTY 可弹出为独立原生 Windows 窗口；弹窗与 IDE 共用同一会话，关闭弹窗不会终止 IDE 或 shell。
- Windows 打包目标默认全不选，并按项目保存选择。
- 已实现真实原生产物、Portable ZIP 和未签名 MSIX；MSIX 使用 Windows SDK `MakeAppx`，完成后打开产物目录。
- 打包成功/失败通过终端标记识别，不再无限转圈。
- 黑洞/白洞使用 Direct2D 手绘同心线和粒子动画，不依赖 CSS/Lottie。

## 5. 尚未完成的任务队列

以下顺序即推荐接手顺序。完成一项后更新本文和测试，不要只改 UI 文案。

### P0：Windows 正确性与实机验收准备

- [x] 在 Windows 11 实机逐项测试：高 DPI、多显示器、窗口最大化/缩放、中文 IME、剪贴板、拖选、保存、新建项目、关系跳转、多终端、Detach、Build/Run、ZIP/MSIX。（**本机不可为，按目标约定交付可执行清单与状态层自检**：`docs/REAL-MACHINE-CHECKLIST.md` 已按当前功能刷新——补上问题面板与严重级别过滤、F2 重命名 / Ctrl+R 引用 / F12 跳转、树行弹簧与编辑高亮（粘贴/撤销）动画、打包吸入与白洞吐出、`Ctrl+Shift+A` AI 面板与历史恢复、MSIX 版本跟随 `Cargo.toml`、扩展运行时候选缓存等条目，并修正"无虚拟化"等过期说明；`tc-ide --self-check` 又补了 6 组断言（打包目标文件往返、打包命令覆盖各目标且无证书时不签名、MSIX 清单身份与显示名、树行弹簧滑动并停稳、编辑高亮合并与上限、吸入轨迹与背景落位），Windows CI 的 `windows` job 会在发布构建后运行它。**实机执行仍待用户在装有 Windows 11 的机器上按清单逐项确认**，结果写回 `docs/KNOWN-ISSUES.md` 与本文件）
- [x] 为独立 ConPTY 窗口补充滚动历史、鼠标文本选择、复制、IME、窗口尺寸到终端行列的同步。（已实现：独立滚动偏移与滚轮、拖选与 `Ctrl+Shift+C`、IME 组合、按实测字宽换算并 `resize`；仍需在 Windows 实机确认高 DPI 与多显示器下的观感）
- [x] 验证关闭 IDE 时弹出终端和 ConPTY 子进程的清理顺序，防止孤儿进程或二次释放。（已实现固定顺序：关闭伪控制台 → 等待 400ms → 必要时 TerminateProcess → 释放句柄；IDE 先销毁弹出窗口再清空会话。实机仍需确认无孤儿进程）
- [x] 修复光标字形宽度偏差：`code_cell` 改为用 DirectWrite 文本布局实测，失败时才回落到经验值。仍待在实机对比光标与选区对齐
- [x] 为 Windows 特有逻辑增加可测试的纯函数，避免所有正确性只依赖远程编译。（已实现：`terminal_core`、`graph_core` 承载网格/滚动/选区/关系图范围与搜索，Linux 上 `cargo test` 覆盖；CI 新增 `windows-typecheck` 与 `format` job）

### P1：编辑器达到可日常使用

- [x] 将文本缓冲升级为适合大文件的 piece table（`text_buffer`）：Unicode 字符边界、分组编辑、按编辑的撤销/重做历史，编辑不再复制整个文档；Windows 编辑器已接入。
- [~] 语义关系不再依赖行正则与括号计数：`syntax_core` 提供语言感知的作用域扫描（注释/字符串屏蔽、括号与缩进块配对、声明归属、调用归属），关系图取用真实作用域；**仍不是 Tree-sitter 级别的完整语法树**（无表达式与类型分析）。（本轮继续推进：调用现在带**接收者**——`self.helper()`、`Engine::new()`、`writer.flush()` 会被解析成 `CallReceiver`（此前 `self.method()` 这类成员调用被整个跳过，因此类内方法调用在图里完全没有边）；`impl`/`namespace` 这类只开作用域不声明的块改为 `scope_hint`（`impl Display for Engine` 归到 `Engine`，且不会让同一个类在图中出现两次）；新增 `DeclarationRef`（声明 + 所属类/函数）、`owner_of`/`declaration_refs` 与 `resolve_call`——按「本类成员 > 本文件 > 其他文件」排序候选并给出 `CallConfidence`；`index_core` 现在为每个文件同时保存符号表与**带归属的声明表**（`declarations()`/`file_declarations()`），`graph_core` 的「被调用者」改用 `resolve_call`，跨文件的被调用者会被找到并标成弱边（`GraphEdge::Weak`），命名冲突时列出最可信的候选而不是随手取第一个。新增 9 项单元测试（接收者解析、归属、同名方法排序、跨文件、上限与去重、索引声明表随编辑更新）。**本轮再进一步**：新增 `LocalBinding` —— `let engine = Engine::new()`、`const writer = new Writer()`、`const helper = Helper()` 这类「局部变量的初值是某个类的构造调用」会被扫描记录下来（`engine = build()` 这种无法命名的初值不猜），`resolve_call` 于是能把 `engine.run()` 解析到 `Engine::run`（本类成员优先），并且只在**该变量的作用域**内生效：兄弟函数里的同名局部不会串味，同名变量重新赋值后以最新的绑定为准，比较表达式（`if engine == Engine::new()`）与字符串里的构造调用都不会被误当成绑定。新增 4 项单元测试（绑定识别、作用域可见性、误判防护、图上的端到端解析）。**仍缺**：一般表达式与类型分析（`let x = make(); x.run()` 这类要等真正的返回类型推导），以及基于语法树的真实增量解析）
- [x] 接入真实 LSP：`lsp_core` 客户端（stdio 管道、分帧、自动应答服务端请求）已在 Windows 侧驱动补全、诊断（行标记与状态栏、光标所在行的诊断信息）、F12 跳转定义、Ctrl+I 悬停、Ctrl+R 引用列表（再次按 Ctrl+R 依次跳转）与 F2 重命名（编辑应用到当前缓冲与其他文件）。仍未实现独立的诊断面板与重命名预览。
- [x] 语言服务器与工具状态：按语言查找 rust-analyzer / clangd / basedpyright / typescript-language-server / gopls / jdtls 以及 rustfmt / clang-format / ruff / biome / gofmt / lldb-dap / debugpy / dlv，并用行为探针报告真实安装状态（Windows 扩展页与 Linux 扩展面板按语言显示 `LSP/格式化/调试` 的 ok 或 missing），不再使用持久化假状态。
- [x] 加入文件脏状态、关闭未保存确认、多文件标签、最近项目和会话恢复。（Windows 已实现标签栏、脏标记、Ctrl+W、原生确认框与 session.conf 恢复；Linux 仍未实现）
- [x] 编辑器滚动条、大文件虚拟化、水平滚动与项目树滚动条均已完成（`Viewport` + `line_window`，渲染器只绘制可见行、裁剪代码列，并按总行数与偏移绘制两侧滚动条）。

### P1：关系图可扩展性

- [x] 增加关系图搜索框和自动范围：悬停符号时自动聚焦该符号；无悬停时以当前文件/当前文件夹为范围，而非始终从项目根铺满。（Windows 已接入共享 `graph_core`：顶部搜索框、范围 chip（项目/文件夹/文件/符号/自动）、`Tab` 或方向键切换范围；Linux 仍是自己的节点模型，尚未迁移到 `graph_core`）
- [x] 增加 `项目 → 文件夹 → 文件 → Class → 函数/变量` 的明确层级、主从关系、同级路径和折叠。（Windows 已按 `GraphNode.depth` 缩进并显示路径:行号；折叠交互尚未实现，见新增队列项）
- [x] 大项目使用后台增量索引、视口裁剪、聚类和层级细节。（`index_core` 按文件指纹只重解析变化的文件并在编辑时原地更新；视口裁剪、节点预算与超出预算的文件夹聚类均已完成。仍缺按缩放切换的层级细节。）
- [x] 补充声明、调用者、被调用者、弱定义/推断边的视觉表达。（graph_core 已分类，两个后端用不同但克制的颜色呈现；所谓 AI 推断目前只是“无调用语法”的启发式，非真正 AI 推断）
- [x] 让跨文件路径点击、键盘导航和当前文件切换保持一致。（Windows 支持上/下选择、回车跳转、空格折叠并按节点目标跳转；Linux 也已迁移到 graph_core）
- [x] 关系图节点折叠/展开：类与文件夹节点可按节点 key 折叠（Windows 点击标记或空格，Linux 点击标记）。
- [x] 把 Linux 的关系图节点模型迁移到 graph_core：删除演示节点表与自建范围/搜索/折叠逻辑，改为真实项目索引 + graph_core。

### P1：扩展与 AI

- [x] 扩展安装：extension_core 已实现下载/校验/解包/卸载命令与运行时缓存清单，Windows 面板执行真实命令并校验摘要；**目录需用户提供（`<项目>/.tc-ide-catalog.conf`），仓库不内置编造的 URL 与校验和**。（本轮补上升级/卸载入口：新增共享 `version_is_newer`（逐段数字比较，`1.2.10 > 1.2.9`，预发布版 `1.0.0-beta` 低于同号正式版，非数字版本按"不同即更新"处理）与 `update_plan`/`UpdateState`/`UpdatePlan`——用运行时缓存（已装）对照目录（可装）算出每个扩展是「已最新 / 可升级 / 目录已不再收录」，并给出可读描述，2 项单元测试覆盖；Windows 扩展面板新增 **`U` 升级**（目录版本更新时走真实下载/校验/解包流程）与 **`X` 卸载**（执行 `uninstall_commands`、从运行时缓存清单删除该条、清掉已安装标记并落盘），上下键切换语言时状态栏直接显示该扩展的更新状态；Linux 的扩展面板用同一个 `update_plan` 显示「N 个可升级」（有目录时）。仓库仍不预置任何 URL 或校验和，安装必须由用户提供的目录驱动）
- [x] 用户级运行时缓存清单：记录 id/版本/校验和/字节数/目录，支持复用、修剪与序列化（Windows 已接入，Linux 待接入）。
- [x] Debugger/Formatter：格式化当前文件已完成；调试是真正的 DAP 会话（F9 断点、F5/Shift+F5、F10、F11/Shift+F11、自动取调用栈/作用域/变量并跳到停下的帧），调用栈、变量与断点现在显示在 `Ctrl+Shift+D` 面板中并可回车跳转。（**本轮补齐最后两项**：①选区格式化——`workspace_core::format_selection_command` 按工具给出带范围的命令（`rustfmt --file-lines` 的 JSON 行区间、`clang-format -i --lines=a:b`），没有范围选项的格式化器（ruff/biome/gofmt）明确回报"整文件重排"而不是假装只改了选区；`Ctrl+Shift+F` 有选区时只格式化选区，没选区时整文件。②断点的就地编辑（增删/条件）——`dap_core::Breakpoints` 成为断点表（按文件、按行、带条件），`BreakpointSpec` 支持 condition/hitCondition/logMessage 并正确转义进 `setBreakpoints` 请求；F9 仍增删断点，新增 **Ctrl+F9 就地编辑光标所在行的条件**（状态栏内联输入，Enter 应用、Esc 取消），`Ctrl+Shift+D` 面板的断点行会显示条件。顺带修掉两个真实缺陷：F2 重命名的内联输入此前**没有任何提交路径**（输入完按回车什么都不发生），现在 Enter 提交、Esc 取消；"格式化当前文件"此前在把命令写进终端后**立刻**重读文件（格式化器还没跑完），现在靠结束标记等待工具真正完成，20 秒无响应给出可读提示。Linux 端本来就没有格式化入口（本次未新增）。新增 3 项单元测试（范围命令、条件断点请求、断点表编辑））
- [x] AI：`ai_core::ChatSession` 子进程 curl（Key 走环境变量、SSE 增量、HTTP 状态、可取消）+ Windows 面板（内联输入、Enter 发送、Esc 取消、流式回答、错误显示、最近 12 轮上下文），会话按项目保存到 `.tc-ide-ai/history.conf` 并在打开项目时恢复。
- [x] API 请求使用用户选择的 Endpoint；Key 仅驻内存并通过子进程环境变量 TC_IDE_AI_KEY 传递，不出现在命令行、配置文件或日志。

### P1：打包与动画

- [x] MSIX 签名：打包面板可选择已有证书并调用 signtool；不生成、保存或上传私钥，受密码保护的证书需手动签名（失败信息会说明）。
- [x] 产物报告：BUILD-RESULT.txt 记录每个目标的路径、大小、耗时与失败命令，状态栏给出摘要。
- [x] 各自系统生成产物：Linux runner 生成 .tar.gz 与真实 .deb（并尝试 AppImage）、macOS runner 生成 .app 与 .dmg、Windows runner 生成 ZIP 与 MSIX，均从各自系统上传到同一个 Release。
- [x] 打包进度绑定真实构建阶段：workspace_core::packaging_progress 按输出中的 Compiling/Finished/Compress-Archive/MakeAppx 等标记推进，绝不倒退。
- [x] 继续优化吸入拉伸、距离加速、白洞吐出和极简平台图标；避免白色方块和无意义连线。（新增共享 `package_motion`：`absorb_seconds` 由项目行数与目标数决定吸入时长、`absorb_window` 实现"距离加速"（越远的源越早出发、速度越快，全部同时抵达）、`absorbed` 返回位置/拖尾/摆动（拖尾与摆动按行程比例缩放，窗口大小与 DPI 变化时观感一致）、`emitted`/`emission_slots` 负责白洞吐出的扇形落点。Linux 的吸入与吐出都改由共享模型驱动，四个产物分两列扇形落位；Windows 的吸入条同样改为共享模型，并且构建完成后不再把源"倒着射回去"当白色方块（改为已吸入即消失、拖尾只画细的 muted 拖痕），源方块也改用 text/muted 而非近白色 bright。新增 `--render-scene packaging|packaging-emit` 让包装动画可以本地渲染成 PNG 检查，CI 的 Linux job 现在会渲染编辑器帧与两个包装动画帧，并要求包装帧与编辑器帧逐字节不同（防止 `--render-scene` 静默失效）。Linux 的产物图标原本就是极简线框，无需修改）

### P1：视觉与交互

- [x] 文件树折叠/展开加入真正的行位置弹簧过渡，不能只旋转箭头后瞬间消失。（共享 `motion_core::RowShift`：按行的稳定标识（项目树用文件路径）保存一条弹簧，布局把行放到新位置时返回的是滑动中的小数行号。Windows 项目树已接入——`update_motion` 每帧重排行位置并放进场景，渲染器按 `scene.tree_row_positions` 绘制：折叠后下方的行滑上去、展开时再滑下来，切换项目会清空弹簧以免从上一棵树上滑过来；`tools/windows_editor_sim.py` 与 Windows 单元测试都覆盖了折叠→滑动→停稳与展开的序列。Linux 端原本就用逐行 reveal 高度累积实现同样的滑动（折叠时子行高度收到 0，下方行随之上移），因此不再改造；共享模型已可直接复用于其他会重排的列表）
- [x] 输入和删除动画扩展到多字符粘贴、撤销/重做，同时保证高频输入不积压动画。（共享 `motion_core::EditPulses`：按编辑类型（插入/删除/粘贴/撤销/重做）记录一段高亮，同一行、120ms 内且范围相接的同类编辑会合并成一段（打一个词只有一次动画），最多保留 4 段（超过就丢掉最旧的），高亮 260ms 内淡出后自动移除，`amount()`/`eased()` 给出淡出量、`on_line()` 给出该行最新的一段、`removes()` 标记删除。Linux 编辑器改用它：输入、粘贴（含多行粘贴）、补全插入、退格、Delete、删除选区都会点亮对应区间，删除没有可抬起的文本，就改用一条随淡出收缩的竖线标记缺口；Windows 编辑器同样改用它（`insert_text` 按字符数区分插入与粘贴、退格/Delete/删除选区记为删除、撤销与重做各自成段），渲染器按该段淡出量抬起代码行。`tools/windows_editor_sim.py` 现在会验证"粘贴是一段、删除/撤销/重做都有标记、连打 40 次不会积压、最新一次仍然高亮"；新增 `--render-scene edit` 让编辑动画也能渲染成帧，CI 的 Linux job 一并渲染该帧）
- [~] 背景图增加可视化裁剪框、拖拽和缩放预览，而不仅是数值调节。**Linux 端已完成、Windows 端待接**：设置面板加高到能放下预览，面板里画出整张图片的缩略预览（用共享 `placement` 把图片按 fit 缩放进去），并把"编辑器当前显示的那一块"直接画成**取景框 + 四角把手**；指针按在框内即可拖动平移、按在角或边把手即缩放，位移会在预览坐标与图片坐标之间换算后交给共享 `drag_crop`，因此拖动时缩放值与取景位置同步更新（状态栏显示取景尺寸、位置与缩放倍数），松手即结束。新增 `--render-scene background-crop` 把这一屏渲染成帧并纳入 CI 帧门槛（本地核对：预览区 641×270 内 61859 个亮点像素、1072 个强调色像素=边框与把手）。数值按钮（方向键/不透明度/缩放）仍保留。**仍缺**：Windows 设置浮层里同样需要"预览 + 取景框拖拽"，而 Windows 目前只有数值按钮（本机无法验证该 UI，已记录在实机清单第 2.6 节）。
- [x] 底部终端面板的鼠标选择与复制。（已实现：面板正文拖选、按单元格高亮、状态栏显示选中字符数、`Ctrl+Shift+C` 复制，`Ctrl+C` 仍中断；仍待在 Windows 实机确认高 DPI 下的对齐）
- [~] 设置、打包与项目树等可点击行已有悬停高亮；**按钮的按下/禁用/键盘焦点状态与浅色主题对比度检查仍未完成**。（对比度检查已完成（见 `theme_core`）；**控件状态本轮完成大半**：共享 `theme_core::ControlState`（正常/悬停/按下/禁用）与 `ControlVisual`——按下会下沉并变暗、悬停提起并让边框沾一点强调色、禁用保持形状但文字降到 muted 并整体变淡，配 `mix()` 混色与 2 项单元测试（各状态必须彼此可辨、禁用文字仍可读、每个状态的标签与底色对比 ≥3）；Linux 设置面板的 14 个控件（语言/主题/选择 PNG/清除/不透明度 ±/取景方向/缩放 ±）现在**共用一份矩形定义**，绘制、命中测试与键盘焦点不会各写一套，并且：按下有按下态、不可用的控件（无图片或已到边界）显示禁用态并在点击时说明原因、`Tab`/`Shift+Tab` 在可用控件间移动焦点并画出强调色焦点环、`Enter`/`Space` 激活、`Esc` 退出焦点；Windows 侧新增按住状态跟踪（`pointer_pressed`），打包浮层的目标行改用共享 `control()` 呈现悬停/按下，并新增 `msix_supported()` 让"MSIX 需要 Windows 可执行文件"这条规则同时决定行的禁用态与命令的拒绝理由。新增 `--render-scene settings-hover`/`settings-controls` 两个帧并纳入 CI（本地核对：悬停填充 #38383a 且边框 255 个强调色像素、按下填充 #000000、焦点环 264 个强调色像素、禁用项为安静边框 + muted 文字）。**仍缺**：Windows 设置/扩展浮层的行与按钮还只有悬停态（没有按下/禁用/焦点环），键盘焦点环目前只有 Linux 有）
- [x] 设置与扩展面板的完整中文文案已完成（设置各行、扩展类别、安装/检测提示、打包目标与证书行、关闭提示）。

### P2：macOS 与共享架构

- [~] `macos_app.rs` 目前远落后于 Windows/Linux；实现 Cocoa 原生窗口、CoreText/Metal、NSPasteboard、NSOpenPanel、PTY 和 NSWorkspace。**Cocoa 窗口、绘制、编辑、剪贴板、打开面板、访达显示与菜单栏均已落地**（不经绑定 crate，直接走 Objective-C 运行时；零依赖）：`NSWindow` + 自注册的 `NSView` 子类（`drawRect:`/`keyDown:`/`insertText:`/`mouseDown:`/`isFlipped`/`acceptsFirstResponder`）+ 60Hz `NSTimer`；标题栏/行号栏/代码列/选区/呼吸光标/状态栏都会绘制，**字宽改为 AppKit 实测**（`sizeWithAttributes:` 量十个数字再取平均，按字号缓存，失败才回落到经验值），光标与选区因此与字形对齐；**`NSPasteboard` 复制/剪切/粘贴**（`public.utf8-plain-text`）、**`NSOpenPanel` 打开文件（新标签）与打开文件夹**、**`NSWorkspace` 在访达中显示**，并以 `NSMenu` 建起应用菜单（Quit）与 File/Edit 菜单，菜单项与快捷键共用同一张命令表（`command_for_menu_action`），不会出现菜单和快捷键做两件事；编辑行为来自共享 `editor_session`（本轮补上选区文本/剪切/粘贴、按编辑分组的撤销与重做、按文件打开新标签、关闭标签），其 13 项单元测试在 Linux 上即可运行，macOS 自检也调用同一份 `state_check`。**标签栏也已实现**：标题栏画出每个打开的文档（活动标签高亮、脏状态点由会话给出），点击标签切换、`⌘⇧]`/`⌘⇧[` 前后切换（循环）、`⌘W` 关闭（最后一个保留），File/Edit 菜单里也有对应项，命中测试与绘制共用同一个 `tab_index_at`/`TAB_WIDTH`。**PTY 终端也已落地**：用 `forkpty` 起真正的登录 shell（`$SHELL`），后台线程读主设备并只把完整字符交给解析器（共享 `terminal_core::Utf8Chunker`，跨读边界的多字节字符不会被拆坏），输出喂给共享 `TerminalParser`/`TerminalView` 并以 `terminal_core::panel_metrics` 画成底部抽屉（`⌘J` 开关、PageUp/PageDown 翻历史、抽屉用共享弹簧动画），键盘在终端获得焦点时全部走终端（`terminal_core::key_sequence` 给出方向键/Home/End/Delete/Tab/Enter/Escape 的固定序列，可打印字符走 `insertText:` 以便 IME 先合成），窗口尺寸变化会按网格调用 `ioctl(TIOCSWINSZ)` 同步，退出应用前先关闭 shell 与句柄。**关系图面板也已落地**：右侧面板由共享 `graph_core` 驱动（把打开的文件当作索引：每个文件的声明表来自 `syntax_core::scan`+`declaration_refs`，符号来自 `editor_core`，光标所在符号即焦点），范围/关系/折叠/选择全部走共享模型；`⌘G` 开关、`⌘⇧G` 切范围、上下键移动选择、空格折叠、Tab 切关系、回车跳到节点目标、Esc 交回键盘给编辑器，点击面板行即选中该行，面板几何（首行位置、行高、页脚、可见行数）直接用 `graph_core` 的共享常量与 `visible_rows`，因此与其他两个后端的排版一致；**macOS 自检会在 runner 上构建一份真实的关系图并断言**：项目根与 Class 节点存在、`self.prepare()` 被解析到所属类、折叠文件后符号被隐藏、选择夹取、回车跳到目标行、范围与关系循环回到起点。**仍缺**：Metal 渲染与直接 CoreText 排版、设置/扩展/打包面板；这些需要在后续轮次继续，且最终仍需真机确认观感）
- [x] 共享模型下沉：工作区（`workspace_core`）、设置、项目模板、会话（`session_core`）、终端（`terminal_core`）、关系图（`graph_core`）、AI/扩展/LSP/DAP 与动画模型（`motion_core`）都已下沉。（本轮补上最后两块：打包目标选择的读写（`PACKAGE_TARGETS_FILE` + `parse_package_targets`/`serialize_package_targets`，Windows 三个目标与 Linux 四个目标共用同一份文件格式）、完整打包命令拼装（`PackageRequest`/`package_command`：Rust/Go/C/C++/Python/Java 的构建步骤、ZIP、`MakeAppx`、`signtool` 签名与结束/失败标记）、MSIX 清单（`msix_identity`/`msix_display_name`/`msix_manifest`/`MSIX_ASSET_FILES`，清单版本改为随 `CARGO_PKG_VERSION` 变化，此前写死 0.2.0.0），以及设置/会话文件名常量。Windows 与 Linux 后端都已改为调用共享实现；`windows_app.rs` 里不再有自己的拼装逻辑）
- [x] 把 `windows_app.rs` 拆为事件/编辑器/工作区/扩展/打包/终端窗口模块；把绘制按场景区域拆分。（`windows_app.rs` 从 7131 行降为 1359 行的模块根：只留 Win32 声明、常量、应用状态与生命周期，其余拆到 `src/windows_app/` 下的 14 个模块——`events`（窗口过程与命令分发）、`keys`、`pointer`、`editor`、`workspace`、`language`、`extensions`、`packaging`、`debug`、`ai`、`panel`、`detached`、`scene`、`motion`；`windows_renderer.rs` 中 2651 行的 `render` 拆为共享 `ScenePaint`（画笔与几何）加 8 个场景区域绘制方法：终端面板、打包进度、打包浮层、设置、扩展、问题、AI、新建对话框。`tests/module_layout.rs` 守卫这次拆分：模块文件必须存在且各带模块注释、单文件不得超过 1000/1500 行、每个场景区域必须有独立绘制方法并在 `render` 中被调用）
- [x] 平台边界：`tests/platform_boundary.rs` 会拒绝共享核心里的平台导入、窗口句柄、原始 FFI 类型与 `unsafe` 块，检查每个共享核心都在 `main.rs` 中声明（防止守卫漏掉新模块），并断言三个平台模块仍处于 `cfg(target_os)` 之后。

### P2：自动化与发布前门槛

- [x] 可脚本化的状态层测试：tc-ide --self-check 覆盖共享内核断言，Windows CI 在发布构建后运行它。
- [x] 单元测试覆盖：`graph_core`、`terminal_core`、`editor_core`、`workspace_core`（偏好设置/项目模板/打包命令与打包目标/打包报告）、`session_core`、`ai_core`、`extension_core`、`lsp_core`、`dap_core`、`text_buffer`、`index_core`、`syntax_core`、`motion_core`（含行位置弹簧）均已覆盖（本地 233 项共享内核测试 + 4 项平台边界测试 + 3 项模块结构测试；Windows 侧另有编辑器与项目树行弹簧的测试）。
- [x] CI 加入 `cargo fmt --check`、Clippy（先清零现有警告）和产物结构检查。（`cargo fmt --check` 与 `windows-typecheck` 已加入；Clippy 仍待清零现有警告后接入，当前 Linux 侧仍有 18 条、Windows 侧 2 条 dead_code 类警告）
- [x] 版本号（Cargo.toml 0.3.0）、CHANGELOG.md、docs/INSTALL.md、docs/KNOWN-ISSUES.md、docs/REAL-MACHINE-CHECKLIST.md 已就绪，均标注“未发布”。
- [x] 未触发 release.yml、未创建标签或 Release；工作流已补齐三平台产物步骤，但仍然只能手动触发。

## 5.1 仍然开着的队列（2026-10-15 之后）


图例：`[x]` 已完成，`[~]` 只完成了一部分（详情写在条目里）。下面是仍然需要继续的具体工作，按建议顺序：

1. **Windows 实机验收**：按 `docs/REAL-MACHINE-CHECKLIST.md` 逐项确认，把结果写回本文件与
   `docs/KNOWN-ISSUES.md`。这是当前唯一无法由 CI 或 Linux 本地替代的部分。
2. **LSP**：补全、诊断（行标记、状态栏、`Ctrl+Shift+D` 问题面板与严重级别过滤）、F12 跳转、Ctrl+I 悬停、F2 重命名（写入当前缓冲与其他文件）、Ctrl+R 引用列表都已接入，语言服务器与工具状态按语言做真实探针；剩下的是把语义关系推进到完整语法树（见第 4 项）。
3. **编辑器剩余部分**：piece table、虚拟化、编辑器与项目树滚动条、水平滚动均已完成。
4. **Tree-sitter 或等价增量解析**：替换正则/启发式语义关系。
5. **调试面板**：调用栈与变量已经有数据（`dap_core::DebugState`），缺的是一个独立面板与断点管理界面。
   诊断同理：数据已在 `lsp_diagnostics`，缺独立面板。
6. **AI 会话**：面板、流式回答、最近 12 轮上下文与按项目的历史持久化都已完成（`Ctrl+Shift+A`）；剩下的是让推断边变成真正的模型推断，而不是“无调用语法”的启发式。
7. **关系图**：增量索引已完成；下一步可以是把 `graph_core` 的层级细节与聚类做得更细（按视口密度切换层级）。
8. **视觉细节**：悬停高亮与完整中文文案已完成；仍缺文件树行位置弹簧、多字符粘贴/撤销动画、
   背景图可视化裁剪框、按钮按下/禁用/键盘焦点状态与浅色主题对比度检查。
9. **macOS 原生后端**：Cocoa/CoreText/Metal、NSPasteboard、NSOpenPanel、PTY、NSWorkspace。
10. **架构收尾**：`windows_app.rs` 的模块拆分与绘制区域拆分已完成（见 P2 条目），
    Linux 已接入会话恢复与运行时缓存清单；剩余的是下沉偏好设置/项目模板/打包命令的重复实现。

## 6. 下一位开发者的第一批任务

第 1–5 项（终端滚动/选择/复制/网格同步、关系图范围与搜索）已完成并推送，基线见第 4 节。建议接下来按以下小批次继续，避免同时改动所有后端：

1. 在 Windows 实机验收本批次：高 DPI、多显示器、弹出终端滚动与选择、`Ctrl+Shift+C` 与 `Ctrl+C` 的区别、IME、终端网格换行是否正确。
2. 关系图折叠：类/文件夹节点可折叠，并与当前文件切换保持一致。
3. 把 Linux 关系图迁移到 `graph_core`，删除 `linux_app.rs` 里重复的范围/搜索实现。
4. 把偏好设置解析、项目模板和打包命令下沉为共享纯函数并补单元测试（现在只能在 Windows job 或实机上验证）。
5. 每完成一个批次执行本地测试、`cargo fmt --check`、Windows 类型检查、推送并等待 Windows job 成功。

不要先做新的视觉概念页。

## 7. 代码风格与实现规范

### Rust

- 使用 Rust 2021，提交前执行 `cargo fmt`。
- 优先把可测试逻辑写成纯函数，平台消息循环只负责分派。
- `unsafe` 必须缩小作用域，并在边界处完成空指针、长度、句柄所有权检查。
- Win32 字符串统一使用 UTF-16 NUL 结尾；不要把 Rust `&str` 指针直接交给 Win32。
- 不在 UI 线程执行下载、索引、大项目扫描或长时间打包；这些操作应进入后台任务/终端，会通过状态消息反馈。
- 路径必须使用 `Path`/`PathBuf`，避免手写跨平台分隔符；只有生成目标 shell 命令时才做平台转义。
- 新增语言时同时检查：后缀识别、注释、关键字、版本、项目模板、Build/Run、扩展 UI 和测试。

### UI 与动画

- 使用语义颜色：背景、抬升层、正文、弱文字、分隔线、选择态、强调色；不要为每个控件随意增加颜色。
- 所有尺寸乘 DPI scale；点击区域必须和绘制坐标使用同一套公式。
- 动画使用单调时钟和弹簧/缓动状态，不允许阻塞式 sleep，不为每个控件启动线程。
- `prefers-reduced-motion` 在桌面端尚未实现；后续应提供“减少动画”设置。
- 不用字母 `V` 代替下拉箭头；应绘制箭头图形或使用明确文本。
- 用户可见按钮必须有真实动作；未完成时显示明确状态，不能点击后无反馈。

### Git 与提交

- 一个提交完成一个可验证的小功能，示例：`feat: ...`、`fix: ...`。
- 推送前运行：

Windows 独有的编辑逻辑也可以在 Linux 上直接跑一遍：`python3 tools/windows_editor_sim.py` 会把
`windows_app.rs` 的编辑方法抽取到临时 crate，链接真实的 `text_buffer`/`editor_core` 并驱动插入、换行、
退格、剪切、撤销/重做、Unicode 与补全序列。它是排查工具（真正的测试在 Windows 测试模块与 CI 中），
本轮正是靠它定位到「剪切没有关闭编辑组导致无法撤销」的真实缺陷。

```bash
cargo fmt --check
cargo test --all-targets
git diff --check
```

- 推送后检查 `.github/workflows/build.yml` 的 Linux、Windows、macOS 三个 job。
- Windows 原生接口改动必须等待 Windows job 完成；Linux 本地测试不会编译 `cfg(windows)` 代码。
- 工作区有用户改动时不得覆盖或重置。

## 8. 构建与验证

### Linux 本地

```bash
sudo apt install build-essential libx11-dev libcairo2-dev libpango1.0-dev
cargo run --release
```

### Linux 上校验 Windows 代码（不链接）

```bash
rustup target add x86_64-pc-windows-msvc
cargo check --all-targets --target x86_64-pc-windows-msvc
cargo fmt --check
cargo test --all-targets   # 覆盖 graph_core / terminal_core / editor_core
```

### Windows

```powershell
cargo test --all-targets
cargo run --release
```

测试 Detached 终端时：打开底部终端，执行一个持续输出命令，点击 `Detach`，确认两个界面看到同一输出；在弹窗输入命令并用 `Ctrl+Tab` 切换 Build/Run/Output。

### CI

普通 push 会执行：

- Ubuntu 24.04（format）：`cargo fmt --check`
- Ubuntu 24.04（linux）：`cargo check --all-targets` + `cargo test --all-targets`
- Ubuntu 24.04（windows-typecheck）：`cargo check --all-targets --target x86_64-pc-windows-msvc`
- Windows 2025：`cargo test --all-targets` + `cargo build --release`
- macOS 15：`cargo build --release`

## 9. 已知文档偏差

- `README.md` 和 `docs/ARCHITECTURE.md` 的部分“待实现”描述早于当前 Windows 代码，例如 Windows DirectWrite/Direct2D、ConPTY、MSIX 已经实现到更后阶段。
- README 中部分 Linux 打包描述仍属于原型状态，不能据此宣称真实跨平台安装包均已完成。
- 后续完成一个大功能后，应同步更新 README、ARCHITECTURE 和本文，避免再次出现实现与说明不一致。

## 9.1 本批次的验证方式（可复现）

```bash
rustup run stable cargo fmt --check                          # 必须用 stable 的 rustfmt：本机 /usr/bin/rustfmt 1.7
                                                             # 与 CI 的 1.10 排版结果不同，用错版本会“本地通过、CI 失败”
cargo check --all-targets --target x86_64-apple-darwin       # macOS 后端类型检查（本机即可）
cargo clippy --all-targets --target x86_64-apple-darwin -- -D warnings
cargo test --all-targets                                   # 233 项共享内核测试 + 4 项平台边界测试 + 3 项模块结构测试
DISPLAY=:0 cargo run -- --render-frame /tmp/frame.png --render-scene packaging   # 包装与编辑动画也可渲染成帧
DISPLAY=:0 cargo run -- --render-frame /tmp/frame.png --render-scene edit
DISPLAY=:0 cargo run -- --render-frame /tmp/frame.png --render-scene background   # 连背景图位置一起渲染
cargo clippy --all-targets -- -D warnings                   # 共享内核 + Linux 后端
cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings
cargo check --all-targets --target x86_64-pc-windows-msvc    # Windows 类型检查
DISPLAY=:0 cargo run -- --render-frame /tmp/frame.png        # 渲染一帧用于视觉/回归对比
```

CI 对应 job：`format`（fmt + clippy）、`linux`（编译测试 + Xvfb 渲染一帧）、
`windows-typecheck`（类型检查 + clippy）、`windows`（测试 + 发布构建 + `--self-check`）、
`macos`（发布构建）。发布工作流保持手动触发。

## 10. 当前交接结论

TC-IDE 已经从静态概念预览进入可编译的多平台原生原型阶段。Windows 后端拥有编辑、项目树、语义关系、注释气泡、设置/扩展、多 ConPTY、独立共享终端窗口、Build/Run、ZIP 和未签名 MSIX 等真实路径；但它仍缺少 Windows 实机全面验收、成熟文本缓冲、Tree-sitter/LSP、真实扩展下载、DAP、完整 AI Agent、签名安装包和大项目性能验证。

2026-10-15 的终端批次与关系图批次之后，Windows 后端的终端（网格/滚动/选择/复制/IME/弹出共享窗口）与关系图（范围/搜索/层级/点击跳转）已经从"演示状态"推进到有共享核心与单元测试支撑的实现，但两者都还没有经过 Windows 实机验收，Linux 关系图也尚未迁移到共享模型。

第二批工作之后（共享内核、会话与标签、AI/扩展真实链路、打包产物与签名、LSP 协议层、
自动化与发布门槛文档），两个后端的可见功能都建立在有单元测试的共享模型之上：本地测试 115 项，
Windows 代码在 Linux 上可做类型检查与 Clippy，Linux 端能在无窗口模式下渲染一帧做回归对比，
Windows 侧有可脚本化的状态层自检。仍然缺少的、且必须由人或真机完成的部分集中记录在
`docs/KNOWN-ISSUES.md` 与第 5.1 节。

因此当前正确表述是：**Windows 与 Linux 原生功能已大幅推进并持续通过 CI，但项目尚未完成
（缺少实机验收、LSP/DAP 接入、文本缓冲升级、macOS 后端等），也尚未获准发布。**
