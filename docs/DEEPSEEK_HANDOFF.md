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

> **先读这一条：本机的默认 `cargo` 是 1.75，而 CI 用 stable（1.99）。**
> 必须用 `~/.cargo/bin/rustup run stable cargo ...`，否则格式化结果与 CI 不同。

```bash
R=~/.cargo/bin/rustup
$R run stable cargo fmt --check
$R run stable cargo test --all-targets
$R run stable cargo clippy --all-targets -- -D warnings
$R run stable cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings
$R run stable cargo clippy --all-targets --target x86_64-apple-darwin -- -D warnings
python3 tools/windows_editor_sim.py
```

原始命令（`cargo fmt --all` 等）在默认工具链下即可运行，但**不要用它判断 CI 是否会红**。

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

## 10. 接手后的进展（2026-10-17）

### 已完成

- **共享菜单模型 `src/menu_core.rs`**（第 19 个共享核心，已纳入 `tests/platform_boundary.rs` 守护）。
  8 个菜单标题、43 个菜单项、双语标签、快捷键，全部可在无窗口下测试（7 项测试）。
  写这个模型时测试抓出我自己的 3 个建模错误：撤销/重做同时列在「文件」和「编辑」、
  停止同时列在「运行」和「调试」、以及一个项漏登记。
- **Linux 菜单栏按模型绘制并全部接通**。此前「运行」「调试」是画了边框和文字但没有状态字段、
  也没有命中测试的死按钮，点下去什么都不会发生；「窗口」「帮助」根本不存在。
  现 8 个标题全部可点，命中测试与绘制共用同一套矩形。渲染实测：x=202..1165 共 8 个相邻按钮，
  与命中测试的布局一致（差值 ≤3px，即描边本身）。
- 「文件」菜单保留自己的行点击处理：它的行比模型列出的更丰富（分节标题、最近项目、新建项）。

### 顺手修掉的、推送前就已存在的问题

这些问题在 main 上已经存在，会阻塞验证：

1. `src/tcide_project.rs` 一处 clippy 报错（`field_reassign_with_default`）。
2. `src/windows_app/workspace.rs` 使用 `tcide_project` 却没有导入，且 `language` 类型不匹配
   （`&str` 传给 `String` 字段）。
3. **`graph-callees` 渲染场景没有真正生效**，产出的帧与普通编辑器帧**逐字节相同**（都是 597119 字节），
   因此 CI 帧门槛一直是失败的。根因：场景从「当前恢复的会话」取文档，而该会话是 `main.c`
   （6 行、1 个调用、0 个带接收者的调用），查找返回空 → `prime_scene` 报「未知场景」→
   画出普通编辑器帧。现在场景自带文档并主动打开面板；修复后该帧 667460 字节 vs 编辑器 600125 字节，
   本地跑完整帧门槛 **PASS**。

**CI 现已 passing，基线为 `328 + 3 + 4`。**

### 第二批已完成（2026-10-17）

- **菜单键盘导航与悬停**：左右环绕切换标题、上下在菜单内移动（到端点停住而不是回绕）、Enter 执行、
  Esc 关闭；指针悬停高亮与键盘共用同一个光标，两者不会显示不同高亮。下拉菜单由模型统一绘制并带滑入动效。
  测试 6 项。
- **逐字输入/删除动画**：每个字符在编辑生命周期内有自己的时刻，先从下方升入再达到全亮，
  取代原来「整行一起抬升」。删除没有字符可带入，保留原来的间隙标记。测试 5 项。
- **动效曲线按角色统一**：新增 `MotionRole`（Overlay / Drawer / Follow），各自一套弹簧与行程；
  原本九个面板共用一个曲线。测试 5 项。
- **文件标签关闭/新建 + `.tcIDE` 点击进关系图**：标签由共享几何布局，绘制与命中测试同源；
  关闭后选中合理的相邻标签、最后一个标签不可关闭；打开 `.tcIDE` 恢复其记录的范围/折叠/聚焦文件并显示关系图。
  测试 4 项。
- **面板比例命名化**：项目树宽度原本是 `0.17` 在 **18 处**重复书写，现集中为一个 `Layout`，
  并保证编辑器始终保有不低于 28% 的宽度。重构以「渲染帧逐像素不变」验证。测试 4 项。

**注意：动效调参前先量。** 数学上的稳定阈值（0.0008）让每个动效看起来要 760ms，
我差点据此去调本来没问题的弹簧；实际人眼感知的是前 90%，overlay 只需 150ms。

### 下一步（按优先级）

1. **把同一套菜单语义同步到 Windows 后端**（`menu_core` 已就绪，Windows 尚未接入）。
2. **P0 面板/终端/关系图的过渡曲线统一**到 `motion_core`，避免每处各自一套速度。
3. **P1 `.tcIDE` 点击进入关系图视图**；AI 上下文优先读 `.tcIDE`。
4. **P1 逐字输入/删除反馈**（目前是整行抬升）。
5. **P2 把同一套菜单语义同步到 Windows 后端**（`menu_core` 已就绪，Windows 尚未接入）。
