# TC-IDE 架构草案

## 原则

TC-IDE 不是网页套壳。界面状态、布局、动画和绘制都由 Rust 控制；CSS 只属于早期概念预览，不进入产品运行时。

## 分层

目标分层（括号内为当前仓库中已存在的共享模块）：

1. `tc_workspace`：项目、目录、文件、符号索引。
2. `tc_semantic_graph`：项目 → 文件夹 → 文件 → Class → 函数/变量，以及调用、声明、弱定义与 AI 推断边（`src/graph_core.rs` 已承载范围、搜索、层级与视口裁剪）。
3. `tc_editor`：文本缓冲、选择区、光标和字形级插入/删除动画（`src/editor_core.rs` 已承载后缀识别、注释、关键字、补全、符号与声明分类）。
4. `tc_ui`：原生窗口、GPU 场景、输入与统一动画时钟。
5. `tc_packager`：Windows、Debian、macOS、AppImage、RPM 构建任务和黑洞/白洞状态动画。
6. 终端模型（`src/terminal_core.rs`）：网格换算、滚动历史、选区、文本提取与 VT 解析器，平台层只负责 ConPTY 会话和绘制。
7. `src/session_core.rs`：打开文档、当前标签、脏状态、最近项目与会话恢复文件。
8. `src/workspace_core.rs`：偏好设置解析、项目模板、Build/Run 与格式化/调试命令、打包目标与产物报告。
9. `src/ai_core.rs` 与 `src/extension_core.rs`：AI 请求/响应与错误说明；扩展目录、下载校验解包卸载命令与运行时缓存清单。
10. `src/lsp_core.rs`：LSP 分帧、请求构造与诊断/补全/悬停/位置/重命名解析（传输层尚未接入）。

共享模块不引入任何平台 API：`graph_core`、`terminal_core`、`editor_core` 在三个平台上都参与编译，并由 Linux 上的 `cargo test` 覆盖；Win32 后端另外在 Linux runner 上通过 `cargo check --target x86_64-pc-windows-msvc` 做不链接的类型检查，避免 Windows 特有逻辑只能依赖远程实机编译。

## 平台边界

TC-IDE 不把 Linux 可执行文件改名后当作 Windows 或 macOS 包。入口按目标系统编译：

- `linux_app`：现有 X11/Cairo/Pango 后端；
- `windows_app`：Win32 原生窗口后端，已接入 DirectWrite/Direct2D、Windows Clipboard、原生文件对话框、ConPTY 与 Shell API；
- `macos_app`：独立的 Cocoa/Metal 后端入口，后续接入 NSPasteboard、NSOpenPanel 与 PTY；
- 工作区、文本缓冲、语义图、动画状态和打包任务将逐步下沉为共享的纯 Rust 核心，不允许调用平台 API。

每一项桌面能力都通过平台适配器提供，不能从共享核心直接调用 `X11`、`Win32`、`Cocoa`、`bash`、`cmd.exe` 或系统文件选择器。普通 CI 在 Ubuntu、Windows 和 macOS 上分别编译；只有三个后端达到功能一致并经过对应系统运行测试后，才允许正式发布各自安装包。

| 能力 | Linux | Windows | macOS |
| --- | --- | --- | --- |
| 原生窗口 | X11（已实现） | Win32 基础窗口（已建立） | 独立入口（待实现） |
| 文本/GPU 渲染 | Cairo/Pango | DirectWrite/Direct2D（基础编辑画布已接入） | CoreText/Metal（待实现） |
| 系统剪贴板 | X11 selection | Windows Clipboard UTF-16（平台层已接入，待编辑器共享核心） | NSPasteboard（待实现） |
| 文件选择 | zenity（将替换为适配器） | GetOpenFileNameW（已接入） | NSOpenPanel（待实现） |
| 集成终端 | shell/PTY | ConPTY + PowerShell 输入输出流、滚动历史、选择复制、弹出共享窗口（已接入） | PTY + 用户 shell（待实现） |
| 打开输出目录 | xdg-open/gio | Explorer/ShellExecuteW（已接入） | NSWorkspace/open（待实现） |
| 安装包 | deb/AppImage（待接入真实打包） | Portable ZIP 与未签名 MSIX（已生成，签名待接入） | dmg/pkg（待实现） |

Windows 后端还启用了 Per-monitor DPI V2，并处理 `WM_DPICHANGED` 推荐窗口尺寸。其原生菜单和快捷键会实际执行文件选择、UTF-16 剪贴板读写与 Explorer 打开目录。Direct2D 将项目树、可编辑代码、调用层级和底部终端统一绘制；文本模型保留跨行选区、Unicode 光标、200 步撤销/重做和 IME 组合文本。ConPTY 在项目目录启动 PowerShell，通过管道双向传输输入输出，并使用有光标状态的增量 VT 解析器保留未换行提示符。当前仍需在真实 Windows 设备上验证不同 IME、终端全屏应用和字形精确度。

## 关系图的层级语义

```text
TC-IDE project
├── src/editor/graph.rs
│   └── GraphController
│       └── build_graph()
├── src/workspace.rs
│   ├── analyze_workspace()
│   └── resolve_symbols()
└── src/render.rs
    └── render_graph()
```

缩放只改变信息密度，不改变数据结构：远景显示项目和文件，中景加入 Class，近景显示函数、调用方向和声明位置。指针移动到代码符号或图节点后，焦点自动切换。

范围由 `graph_core::auto_scope` 决定：用户显式选择的范围优先，其次是搜索，其次是悬停符号，最后回退到当前文件；`graph_core::build_graph` 只产出节点（层级、缩进、详情、跳转目标），每个后端自行决定卡片样式与绘制方式。节点总数受 `MAX_NODES` 限制，绘制只取当前视口内的节点。

## 项目关系文档（`.tcIDE`）

每个打开的项目根目录会有一个 `<项目目录名>.tcIDE` 文件。它是可读的纯文本索引，不是编译产物，包含：

- `[project]`：项目名称、语言、版本和环境；
- `[files]`：项目内可分析文件及其语言；
- `[symbols]`：声明与引用的文件、名称、行号和角色；
- `[relations]`：引用/调用关系及置信度；
- `[view]`：关系图范围、当前文件、当前符号和折叠状态。

`src/tcide_project.rs` 负责发现、读写、转义和版本兼容。打开项目时读取已有文档，索引刷新后回写；因此右侧关系图和 AI 项目上下文使用同一份项目级事实。未来升级格式时只增加字段或段落，解析器会忽略未知内容。

## 性能目标

- UI 主线程只做输入、布局提交与绘制；
- 项目索引、AI 整理和打包在后台任务执行；
- 动画使用单调时钟和统一场景插值，不为每个组件创建独立计时器；
- 大图采用视口裁剪、层级细节与增量布局；
- 文本变化只更新受影响的字形实例。
