# TC-IDE

![TC-IDE TCIDE icon](assets/tc-ide-icon.png)

TC-IDE 是一个 Rust 原生、低色彩噪声、强调空间关系与丝滑动效的 IDE 实验项目。它沿用 Zed 的核心产品方向：原生窗口、每帧统一绘制、后台语义分析，以及不依赖 HTML/CSS 的界面。

当前第一个可运行原型包含：

- 原生 X11 窗口与 60 FPS 动画循环；
- 根据显示器分辨率自动设置窗口尺寸与界面缩放；文字由 Pango/Fontconfig 渲染，并随程序内置 JetBrains Mono；
- 双缓冲场景绘制，减少持续动画时的闪烁；
- 右侧使用正交树展示 `项目 → 文件 → Class → 函数` 层级，避免蜘蛛网式交叉；
- 鼠标悬停任意代码标识符时，同文件声明/调用沿编辑器左侧轨道连接，跨文件关系指向左侧路径列表；
- 文件树支持相对路径搜索、项目/文件夹折叠和点击切换，空白处右键可新建文件或文件夹；编辑器支持精确字形宽度光标、跨行方向键、鼠标拖选、`Ctrl+A/C/X/V/S`、系统剪贴板与快捷保存；
- 顶部 `New` 菜单统一提供新建项目、新建文件和新建文件夹，`Edit` 菜单集成保存、全选、复制、粘贴和字号操作；
- 新文件会自动识别后缀；文件语言识别与外部工具扩展安装状态已解耦，未安装 LSP 时仍能正确区分 C 与 C++；
- 编辑器按当前文件语言提供独立关键字补全，并从同语言项目文件中提取符号候选；关键字、字符串和数值已有原生语法高亮；
- 注释语法按文件后缀区分（例如 C/C++ 的 `#include` 不会被当成注释）；气泡支持多行、折叠/展开和精确字符编辑，编辑边界被限制在注释正文内，不会删除注释符号或前方代码；
- 顶部 `Settings / 设置` 可切换中英文、字体和字号，并可选择 PNG/JPEG/WebP 编辑器背景、调整不透明度与取景位置；
- `Extensions` 采用“类别 → 具体扩展 → 版本/配置”三级结构；当前提供 Languages，并为 Themes、Debuggers、Formatters 和 AI / Agents 预留独立入口；
- 内置 C、Python、C++、Java、Rust、TypeScript/JavaScript、Go 等 30 个语言/格式入口，未知格式回退为 Plain Text；
- 新建项目可编辑保存位置，环境可选 `None`；Python `venv` 会实际初始化，版本运行时元数据按版本缓存到 `~/.cache/tc-ide/runtimes/python/`以供新项目复用；
- 底部是可折叠的项目终端，默认提供 Build、Run、Output；加号位于标签左侧，超过四个后进入“更多”列表，也可将终端弹出为当前项目目录下的独立窗口；
- 切换/新建项目后会清空旧项目的语义关系图，避免把上一个项目的调用关系留在右侧；
- 打包目标默认不勾选，每个项目独立保存 `.exe` / `.deb` / `.AppImage` / `.dmg` 选择；吸入动画包含函数与变量，使用距离加速和拉伸扭曲，时长会随项目代码量增加，输出改为极简平台图标；
- 编辑区、项目树与语义图使用同一帧绘制；
- 零网页运行时、零 CSS、零第三方 Rust crate；界面直接调用系统 X11、Cairo、Pango 与 Fontconfig。

## 运行

Ubuntu/Debian 首次编译需要原生图形库：

```bash
sudo apt install build-essential libx11-dev libcairo2-dev libpango1.0-dev
```

背景文件选择和非 PNG 图片转换另外使用 `zenity` 与 `ffmpeg`，独立终端优先使用 `gnome-terminal`，不存在时回退到 `x-terminal-emulator`。

然后运行：

```bash
cargo run --release
```

按 `Esc` 或关闭窗口退出。

## 发布规则

日常开发只提交并推送源码到 `main`，不会生成安装包、版本标签或 GitHub Release。发布工作流仅允许手动触发；只有项目所有者明确确认“正式开始”后才执行正式打包和发布。

## 路线

此原型已验证 TC-IDE 的视觉语言和主要交互，但右侧语义图仍是演示数据，语言“下载”仍是界面状态，Python 缓存目前只记录运行时元数据，尚未实现完整的 LSP/编译器下载。后续阶段会加入真实项目索引、Tree-sitter、LSP、GPU 字形动画和多平台打包器。详见 [架构说明](docs/ARCHITECTURE.md)。
