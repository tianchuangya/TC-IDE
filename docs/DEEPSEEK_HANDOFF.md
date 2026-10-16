# TC-IDE 交接文档（给 DeepSeek）

更新时间：2026-10-17  
仓库：`https://github.com/tianchuangya/TC-IDE`  
工作目录：`/home/tianchuangya/桌面/TC-IDE`

## 1. 当前目标

把 TC-IDE 当前原生 Rust 界面，完整迁移到已确认的整体预览方案，而不是继续只做局部插件页面。

最终界面应包含：

- 顶部 TC-IDE 标识、项目路径和完整菜单：文件、编辑、查看、运行、调试、扩展、窗口、帮助；
- 左侧项目文件树、搜索、折叠和当前项目状态；
- 中央代码编辑器、多文件标签、语法高亮、注释气泡和调用关系指向；
- 右侧项目/文件/函数/变量关系层级；
- 底部可折叠、多标签、真实联动的构建/运行/输出终端；
- 设置、主题、字体、背景、插件和 AI 入口；
- 文件切换、面板展开收起、输入、注释气泡、关系图、终端和打包动画统一使用平滑过渡；
- 亮色/暗色主题保持同一套布局和交互语义。

不要把 HTML 预览当成成品。预览文件只用于确认视觉方向，成品必须在 Rust 原生后端中实现。

## 2. 已完成并已推送

最近提交（从新到旧）：

```text
652463e feat: persist tcIDE graph data in native Linux app
22bf94d feat: animate native file tab switching
78f51c1 feat: bring preview menu and file tabs into native shell
5c9892e feat: persist project graph in tcIDE document
```

已完成内容：

1. `src/tcide_project.rs`
   - 定义 `.tcIDE` 项目文件格式。
   - 支持项目名、根目录、语言、版本、环境、文件、符号、关系和视图状态。
   - 支持发现、读取、写入、版本字段和 UTF-8/换行/制表符转义。
   - 解析器忽略未知字段和未知段落，便于以后升级。

2. Windows 后端
   - `WindowsAppState` 会读取项目 `.tcIDE` 文件。
   - 索引刷新后会写入文件、声明、引用关系和关系图视图状态。

3. Linux 后端
   - 打开项目时读取 `.tcIDE`。
   - `rebuild_project_index()` 后同步写回项目文件。
   - 原生编辑器已经加入预览方向的“运行/调试”入口文字。
   - 编辑器文件标签已经加入基本高亮和切换滑入动画。

4. 原有功能已经存在并应保持：
   - C/C++/Rust/Python/Java/Go 等语言识别；
   - 代码滚动、选择、复制粘贴、保存、撤销重做；
   - Build/Run/Output 终端联动；
   - 右侧关系图的项目/文件夹/文件/符号范围、搜索、折叠和跳转；
   - 注释气泡、关系线、调试入口、插件目录和 AI API 配置；
   - 打包动画和多平台打包命令。

## 3. `.tcIDE` 文件格式

默认路径：

```text
<项目根目录>/<项目目录名>.tcIDE
```

格式示例：

```text
# TC-IDE project document — readable by TC-IDE and AI tools
format=1

[project]
name=Demo
root=.
language=Rust
language_version=stable
environment=cargo stable

[files]
path=src/main.rs	language=Rust

[symbols]
id=symbol:src/main.rs:4:main	file=src/main.rs	name=main	line=4	role=declaration

[relations]
from=occurrence:src/lib.rs:8:main	to=symbol:src/main.rs:4:main	kind=reference	confidence=indexed

[view]
scope=file	focused_file=src/main.rs	focused_symbol=main
folded=folder:src
```

注意：当前写入的关系主要来自索引中的声明/引用匹配；完整调用边仍应继续复用 `graph_core::build_graph()` 的解析结果补齐，不能凭字符串猜测复杂调用关系。

## 4. 当前明确未完成

这是接手后优先级最高的部分：

### P0：整体原生 UI 迁移

- 当前原生界面仍不是预览版的完整排版；只迁移了部分菜单文字和文件标签。
- 顶部菜单目前“运行/调试”主要是视觉入口，尚未形成独立完整菜单状态机。
- 需要把菜单栏的点击、悬停、键盘导航、关闭和动效补齐。
- 项目树、编辑器、关系图、终端的间距和比例要统一到预览方案。

### P0：动效

- 文件标签切换已有基础滑入动画，但需要更完整的内容过渡。
- 需要实现真正可感知的逐字输入/删除反馈，不要只做整行淡入。
- 面板展开收起需要统一弹性曲线，避免每个模块各自一套速度。
- 注释气泡、关系线和右侧节点应在焦点变化时平滑出现/消失。
- 终端 Build/Run/Output 切换要有状态过渡，并且必须与真实命令输出联动。

### P0：预览中的界面元素迁移

- 完整菜单：文件、编辑、查看、运行、调试、扩展、窗口、帮助。
- 文件标签关闭、切换、新建标签。
- 查看菜单控制：项目树、调用关系、底部终端、注释气泡、调试信息。
- 右侧关系图的层级路径、同级/跨文件标识、声明/调用/弱引用类型。
- 底部终端多标签、加号、新建终端、独立窗口入口。
- 设置中亮色/暗色主题、背景、字体搜索、代码字号、语法颜色自定义。

### P1：关系图与 AI

- `.tcIDE` 文件点击后应能直接切换到关系图视图。
- AI 上下文读取 `.tcIDE`，优先使用其中的项目结构和关系，而不是重新猜测。
- 关系图应支持从当前光标/悬停符号聚焦，而不是始终显示整个项目的臃肿图。
- 关系图布局需要保持可读，避免蜘蛛网式无意义连线。

### P1：跨平台

- Linux 版本当前能构建并测试。
- Windows 交叉构建受本机 Rust 版本限制：当前 rustc 是 1.75，而 `windows 0.62.2` 要求 rustc >= 1.82。
- 不能把 Linux/X11 API 引入共享核心。
- Windows 应继续使用 DirectWrite/Direct2D 和 ConPTY；macOS 使用平台自己的窗口/渲染实现。

## 5. 验证命令

在仓库根目录执行：

```bash
cargo fmt --all
cargo test --all-targets
cargo build --release
git diff --check
```

Linux 当前应为：

```text
320 unit tests passed
3 module layout tests passed
4 platform boundary tests passed
```

Release 二进制：

```text
target/release/tc-ide
```

桌面启动器：

```text
TC-IDE.desktop
```

不要在 UI 尚未迁移完成前启动并交付旧版程序给用户测试。

## 6. 代码结构

共享核心：

- `src/editor_core.rs`：语言识别、符号、语义高亮。
- `src/graph_core.rs`：关系图范围、节点、层级、折叠、跳转。
- `src/index_core.rs`：增量项目索引。
- `src/tcide_project.rs`：`.tcIDE` 项目文档读写。
- `src/session_core.rs`：窗口、项目、面板和文档会话状态。
- `src/terminal_core.rs`：终端缓冲、VT 解析、选择和滚动。
- `src/motion_core.rs`：通用弹性/过渡动画。
- `src/theme_core.rs`：亮暗主题、语法颜色和控件状态。
- `src/workspace_core.rs`：项目模板、任务命令、打包命令。
- `src/ai_core.rs`：AI API 请求、会话和历史。

平台后端：

- `src/linux_app.rs`：Linux/X11/Cairo/Pango 原生界面。
- `src/windows_app.rs` 及 `src/windows_app/`：Windows 原生界面和模块。
- `src/windows_renderer.rs`：Windows DirectWrite/Direct2D 渲染基础。
- `src/windows_terminal.rs`：Windows ConPTY 终端基础。
- `src/macos_app.rs`：macOS 平台入口。

## 7. 编码规则

- UI 状态必须进入结构体字段，绘制、命中测试和状态更新使用同一个几何函数。
- 动画统一放在 `motion_core` 或使用已有 `UiMotion`，不要每个按钮单独写一套计时器。
- 共享核心不能调用 X11、Win32、Cairo、Direct2D、ConPTY 等平台 API。
- 所有用户可见文案保持中英文双语：使用现有的 `tr("English", "中文")` 形式。
- 文件路径内部统一使用 `/` 的相对路径；Windows 只在平台边界转换。
- 不要用 CSS/WebView 代替原生 UI；HTML 预览只用于设计确认。
- 不要为了通过测试修改测试断言；先核实真实行为再修代码。
- 不要把 API Key、证书、令牌写入仓库、日志、`.tcIDE` 或命令行。

## 8. 交付规则

- 不创建 GitHub Release，除非用户明确说“正式开始打包/发布”。
- 普通功能完成后可以提交并推送 `main`，但必须说明测试结果。
- UI 未达到整体预览前，不要让用户测试旧界面。
- 最终交付前必须重新执行格式化、测试、Release 构建和 `git diff --check`。
- 最终回答要明确区分：已完成、仅有基础实现、仍未完成。

## 9. 推荐接手顺序

1. 先读本文件、`docs/ARCHITECTURE.md`、`docs/TAKEOVER.md`。
2. 运行测试确认基线。
3. 完成 Linux 原生整体 UI 迁移，作为视觉参考实现。
4. 把同一套状态和布局语义同步到 Windows 后端。
5. 补齐菜单交互、动效和 `.tcIDE` 点击打开关系图。
6. 做真实项目回归：Rust、C++、Python 项目各一个，验证打开、编辑、保存、Build、Run、关系图、终端和 `.tcIDE`。
7. 最后才构建交付包；不要提前创建 Release。
