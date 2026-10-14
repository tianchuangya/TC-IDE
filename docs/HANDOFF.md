# TC-IDE 项目交接与后续任务

> 最后更新：2026-10-15  
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
│   ├── editor_core.rs         # 共享语言识别、注释、语法 token、补全、符号关系与声明分类
│   ├── graph_core.rs          # 共享关系图范围、搜索、层级、视口裁剪和节点预算
│   ├── terminal_core.rs       # 共享终端网格换算、滚动历史、选区和文本提取
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

### 4.6 Windows 终端、运行与打包

- 多个真实 ConPTY 会话；Build、Run、Output 和用户新增终端。
- 终端输入输出、VT/CSI/OSC 基础解析、方向键、Home/End/Delete/Tab。
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

- [ ] 在 Windows 11 实机逐项测试：高 DPI、多显示器、窗口最大化/缩放、中文 IME、剪贴板、拖选、保存、新建项目、关系跳转、多终端、Detach、Build/Run、ZIP/MSIX。
- [x] 为独立 ConPTY 窗口补充滚动历史、鼠标文本选择、复制、IME、窗口尺寸到终端行列的同步。（已实现：独立滚动偏移与滚轮、拖选与 `Ctrl+Shift+C`、IME 组合、按实测字宽换算并 `resize`；仍需在 Windows 实机确认高 DPI 与多显示器下的观感）
- [ ] 验证关闭 IDE 时弹出终端和 ConPTY 子进程的清理顺序，防止孤儿进程或二次释放。（当前实现：弹出窗口 `WM_DESTROY` 释放 GDI 字体并把会话交还 IDE，IDE `WM_DESTROY` 清空会话列表；实机仍需确认无孤儿 `powershell.exe`）
- [ ] 修复实机发现的光标字形宽度偏差；当前 `code_cell` 仍含经验值，应逐步改为 DirectWrite 实际字形测量。
- [x] 为 Windows 特有逻辑增加可测试的纯函数，避免所有正确性只依赖远程编译。（已实现：`terminal_core`、`graph_core` 承载网格/滚动/选区/关系图范围与搜索，Linux 上 `cargo test` 覆盖；CI 新增 `windows-typecheck` 与 `format` job）

### P1：编辑器达到可日常使用

- [ ] 将文本缓冲从 `Vec<String>` 升级为适合大文件的 rope/piece table；保持 Unicode 字符边界正确。
- [ ] 接入 Tree-sitter 或等价增量解析，替换正则/启发式语义关系。
- [ ] 接入真实 LSP：补全、诊断、跳转定义、引用、重命名、悬停文档。
- [ ] 为 C/C++ 正确连接 clangd，为 Python 连接 basedpyright/pyright，为 Rust 连接 rust-analyzer；扩展安装状态必须与真实可执行文件状态一致。
- [ ] 加入文件脏状态、关闭未保存确认、多文件标签、最近项目和会话恢复。
- [ ] 增加编辑器/项目树滚动条、代码大文件虚拟化和水平滚动。

### P1：关系图可扩展性

- [x] 增加关系图搜索框和自动范围：悬停符号时自动聚焦该符号；无悬停时以当前文件/当前文件夹为范围，而非始终从项目根铺满。（Windows 已接入共享 `graph_core`：顶部搜索框、范围 chip（项目/文件夹/文件/符号/自动）、`Tab` 或方向键切换范围；Linux 仍是自己的节点模型，尚未迁移到 `graph_core`）
- [x] 增加 `项目 → 文件夹 → 文件 → Class → 函数/变量` 的明确层级、主从关系、同级路径和折叠。（Windows 已按 `GraphNode.depth` 缩进并显示路径:行号；折叠交互尚未实现，见新增队列项）
- [ ] 大项目使用后台增量索引、视口裁剪、聚类和层级细节，禁止一次绘制全部节点。
- [ ] 补充声明、调用者、被调用者、弱定义/AI 推断边的不同但克制的视觉表达。
- [ ] 让跨文件路径点击、键盘导航和当前文件切换保持一致。（Windows 点击已改为按节点目标跳转；键盘上下选择与 Linux 节点的 `graph_core` 迁移仍待做）
- [ ] 关系图节点折叠/展开：类与文件夹节点可折叠，折叠状态与当前文件切换保持一致。
- [ ] 把 Linux 的关系图节点模型迁移到 `graph_core`，删除 `linux_app.rs` 里重复的范围/搜索实现。

### P1：扩展与 AI

- [ ] 当前 Languages 的“安装”仍主要是持久化状态；实现真实下载、校验、安装、升级和卸载流程。
- [ ] 建立用户级运行时缓存清单，记录来源、版本、校验和与占用空间；同版本项目复用缓存。
- [ ] Debugger/Formatter 当前只做 PATH 探测；接入 DAP、格式化当前文件/选区和错误反馈。
- [ ] AI 当前只有配置表单；增加连接测试、聊天/Agent 面板、请求取消、流式响应、超时和错误显示。
- [ ] API 请求必须使用用户选择的 Endpoint，Key 仅驻内存并避免出现在终端命令和日志中。

### P1：打包与动画

- [ ] Windows MSIX 当前未签名；增加证书选择、签名和签名失败说明，但不得自动生成或上传私钥。
- [ ] `.exe`、ZIP、MSIX 必须显示准确产物路径、大小、耗时和失败命令。
- [ ] Linux 在 Linux runner 生成 `.deb`/AppImage，macOS 在 macOS runner 生成 `.dmg`；禁止从 Windows 假生成这些格式。
- [ ] 将打包任务与动画进度绑定到真实构建阶段，而不是仅按时间推进到 90%。
- [ ] 继续优化吸入拉伸、距离加速、白洞吐出和极简平台图标；避免白色方块和无意义连线。

### P1：视觉与交互

- [ ] 文件树折叠/展开加入真正的行位置弹簧过渡，不能只旋转箭头后瞬间消失。
- [ ] 输入和删除动画扩展到多字符粘贴、撤销/重做，同时保证高频输入不积压动画。
- [ ] 背景图增加可视化裁剪框、拖拽和缩放预览，而不仅是数值调节。
- [x] 底部终端面板的鼠标选择与复制。（已实现：面板正文拖选、按单元格高亮、状态栏显示选中字符数、`Ctrl+Shift+C` 复制，`Ctrl+C` 仍中断；仍待在 Windows 实机确认高 DPI 下的对齐）
- [ ] 所有可点击控件增加悬停/按下/禁用/键盘焦点状态，并检查浅色主题对比度。
- [ ] 设置和扩展面板需要完整中文文案，不只是标题切换。

### P2：macOS 与共享架构

- [ ] `macos_app.rs` 目前远落后于 Windows/Linux；实现 Cocoa 原生窗口、CoreText/Metal、NSPasteboard、NSOpenPanel、PTY 和 NSWorkspace。
- [ ] 将工作区、设置、项目创建、终端会话描述、打包任务和动画模型逐步移入共享模块。
- [ ] 把 `windows_app.rs` 拆为事件/编辑器/工作区/扩展/打包/终端窗口模块；把绘制按场景区域拆分。
- [ ] 保持平台模块只调用本平台 API，共享核心禁止出现 Win32/X11/Cocoa 类型。

### P2：自动化与发布前门槛

- [ ] 添加 Windows GUI smoke test 或可脚本化的状态层测试。
- [ ] 对偏好设置解析、项目模板、打包命令、语言注册表、VT 解析、关系图过滤增加单元测试。（`graph_core` 范围/搜索/层级/视口、`terminal_core` 网格/滚动/选区、`editor_core` 声明分类已覆盖；VT 解析仍在 `windows_terminal.rs` 内，需要 Windows job 或抽出纯解析器）
- [x] CI 加入 `cargo fmt --check`、Clippy（先清零现有警告）和产物结构检查。（`cargo fmt --check` 与 `windows-typecheck` 已加入；Clippy 仍待清零现有警告后接入，当前 Linux 侧仍有 18 条、Windows 侧 2 条 dead_code 类警告）
- [ ] 正式发布前准备清晰的版本号、变更记录、安装/卸载说明和已知问题。
- [ ] 只有用户明确说“正式开始”后，才允许触发 `release.yml` 或创建 GitHub Release。

## 6. 下一位开发者的第一批任务

第 1–5 项（终端滚动/选择/复制/网格同步、关系图范围与搜索）已完成并推送，基线见第 4 节。建议接下来按以下小批次继续，避免同时改动所有后端：

1. 在 Windows 实机验收本批次：高 DPI、多显示器、弹出终端滚动与选择、`Ctrl+Shift+C` 与 `Ctrl+C` 的区别、IME、终端网格换行是否正确。
2. 关系图折叠：类/文件夹节点可折叠，并与当前文件切换保持一致。
3. 把 Linux 关系图迁移到 `graph_core`，删除 `linux_app.rs` 里重复的范围/搜索实现。
4. 抽出 `windows_terminal.rs` 的 VT 解析器为纯函数并补单元测试（现在只能在 Windows job 里跑）。
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

## 10. 当前交接结论

TC-IDE 已经从静态概念预览进入可编译的多平台原生原型阶段。Windows 后端拥有编辑、项目树、语义关系、注释气泡、设置/扩展、多 ConPTY、独立共享终端窗口、Build/Run、ZIP 和未签名 MSIX 等真实路径；但它仍缺少 Windows 实机全面验收、成熟文本缓冲、Tree-sitter/LSP、真实扩展下载、DAP、完整 AI Agent、签名安装包和大项目性能验证。

2026-10-15 的终端批次与关系图批次之后，Windows 后端的终端（网格/滚动/选择/复制/IME/弹出共享窗口）与关系图（范围/搜索/层级/点击跳转）已经从"演示状态"推进到有共享核心与单元测试支撑的实现，但两者都还没有经过 Windows 实机验收，Linux 关系图也尚未迁移到共享模型。

因此当前正确表述是：**Windows 原生功能已大幅推进并持续通过 CI，但项目尚未完成，也尚未获准发布。**
