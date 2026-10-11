# TC-IDE 架构草案

## 原则

TC-IDE 不是网页套壳。界面状态、布局、动画和绘制都由 Rust 控制；CSS 只属于早期概念预览，不进入产品运行时。

## 分层

1. `tc_workspace`：项目、目录、文件、符号索引。
2. `tc_semantic_graph`：项目 → 文件 → Class → 函数，以及调用、声明、弱定义与 AI 推断边。
3. `tc_editor`：文本缓冲、选择区、光标和字形级插入/删除动画。
4. `tc_ui`：原生窗口、GPU 场景、输入与统一动画时钟。
5. `tc_packager`：Windows、Debian、macOS、AppImage、RPM 构建任务和黑洞/白洞状态动画。

## 平台边界

TC-IDE 不把 Linux 可执行文件改名后当作 Windows 或 macOS 包。入口按目标系统编译：

- `linux_app`：现有 X11/Cairo/Pango 后端；
- `windows_app`：Win32 原生窗口后端，后续接入 DirectWrite/Direct2D、Windows Clipboard、IFileDialog、ConPTY 与 Shell API；
- `macos_app`：独立的 Cocoa/Metal 后端入口，后续接入 NSPasteboard、NSOpenPanel 与 PTY；
- 工作区、文本缓冲、语义图、动画状态和打包任务将逐步下沉为共享的纯 Rust 核心，不允许调用平台 API。

每一项桌面能力都通过平台适配器提供，不能从共享核心直接调用 `X11`、`Win32`、`Cocoa`、`bash`、`cmd.exe` 或系统文件选择器。普通 CI 在 Ubuntu、Windows 和 macOS 上分别编译；只有三个后端达到功能一致并经过对应系统运行测试后，才允许正式发布各自安装包。

| 能力 | Linux | Windows | macOS |
| --- | --- | --- | --- |
| 原生窗口 | X11（已实现） | Win32 基础窗口（已建立） | 独立入口（待实现） |
| 文本/GPU 渲染 | Cairo/Pango | DirectWrite/Direct2D（待实现） | CoreText/Metal（待实现） |
| 系统剪贴板 | X11 selection | Windows Clipboard UTF-16（平台层已接入，待编辑器共享核心） | NSPasteboard（待实现） |
| 文件选择 | zenity（将替换为适配器） | GetOpenFileNameW（已接入） | NSOpenPanel（待实现） |
| 集成终端 | shell/PTY | Windows Terminal/PowerShell 启动（已接入），ConPTY 流（待实现） | PTY + 用户 shell（待实现） |
| 打开输出目录 | xdg-open/gio | Explorer/ShellExecuteW（已接入） | NSWorkspace/open（待实现） |
| 安装包 | deb/AppImage（待接入真实打包） | MSIX/NSIS（待实现） | dmg/pkg（待实现） |

Windows 后端还启用了 Per-monitor DPI V2，并处理 `WM_DPICHANGED` 推荐窗口尺寸。其原生菜单和快捷键会实际执行文件选择、UTF-16 剪贴板读写、Explorer 打开目录和终端启动，用于在编辑器共享核心迁移前验证平台适配器。

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

## 性能目标

- UI 主线程只做输入、布局提交与绘制；
- 项目索引、AI 整理和打包在后台任务执行；
- 动画使用单调时钟和统一场景插值，不为每个组件创建独立计时器；
- 大图采用视口裁剪、层级细节与增量布局；
- 文本变化只更新受影响的字形实例。
