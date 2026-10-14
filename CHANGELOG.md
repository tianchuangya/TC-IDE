# 变更记录

本文件记录 TC-IDE 的用户可见变化。当前版本号只表示开发阶段，尚未发布正式版本，
也没有创建任何版本标签或 GitHub Release。

## 0.3.0（未发布）

### 共享内核

- 新增 `terminal_core`：终端网格换算、滚动历史、选区与文本提取、VT 解析器、
  Windows 子进程环境块，全部由 Linux 本地单元测试覆盖。
- 新增 `graph_core`：关系图范围、搜索、`项目 → 文件夹 → 文件 → Class → 函数/变量`
  层级、折叠、声明/调用者/被调用者/弱定义/推断边、视口裁剪与节点预算。
- 新增 `session_core`：打开文档、当前标签、脏状态、最近项目与会话恢复文件。
- 新增 `workspace_core`：偏好设置解析与序列化（兼容 Windows 与 Linux 两种键名）、
  项目模板、Build/Run 任务命令、格式化与调试适配器命令、打包目标、打包阶段进度与产物报告。
- 新增 `ai_core` 与 `extension_core`：AI 请求体、端点校验、响应与流式解析、状态码说明，
  以及扩展目录解析、下载/校验/解包/卸载命令和用户级运行时缓存清单。
- 新增 `lsp_core`：LSP 报文分帧、initialize/didOpen/didChange/补全/悬停/定义/引用/重命名
  请求，以及诊断、补全、悬停、位置、重命名编辑的解析。
- `editor_core` 增加声明分类（Class/Function/Variable）与 `let/const/var/static` 变量识别，
  并排除 `if let` 一类流程语句。

### Windows 后端

- 底部终端与弹出终端都保留滚动历史；面板正文支持拖选与 `Ctrl+Shift+C` 复制。
- 弹出终端支持独立滚动、鼠标选择、复制与中文 IME，并按窗口实际尺寸同步 ConPTY 行列。
- ConPTY 网格来自共享几何与真实字体测量，不再写死列数/行数。
- 关系图改由 `graph_core` 驱动：范围 chip、搜索框、层级缩进、折叠标记、
  键盘导航与按目标跳转。
- 编辑器顶部加入文档标签、脏标记、`Ctrl+W` 关闭与原生"未保存"确认；会话（项目、
  打开的文件、最近项目）在下次启动时恢复。
- AI 页面新增提问框、流式发送与连接测试，请求在专用终端中运行并带超时；
  API Key 仅通过子进程环境传递。
- 扩展页新增"格式化当前文件"与"启动调试适配器"，Debuggers/Formatters 仍先做真实 PATH 探测。
- 打包面板新增签名证书选择（`signtool`），打包进度绑定真实构建阶段，
  产物路径、大小、耗时与失败命令写入 `BUILD-RESULT.txt`。
- 代码字形宽度改为 DirectWrite 实测，ConPTY 会话关闭顺序固定（关闭伪控制台 → 等待 →
  必要时终止 → 释放句柄）。
- 新增 `tc-ide --self-check` 状态层自检，Windows CI 会在发布构建后运行它。
- 项目索引移到后台线程执行，扫描期间界面不阻塞；结果所属项目已变化时会丢弃旧结果。
- 关系图在文件数超过预算时把文件夹聚合成 `+N more files` 节点，避免大项目一次铺满。

### Linux 后端

- 关系图不再使用演示数据：改为扫描项目目录（没有项目时使用已打开文件）建立真实索引，
  由 `graph_core` 提供范围、搜索、折叠与关系类型。
- 偏好设置读取改走共享解析器。
- 新增 `--render-frame <path>`：无需映射窗口即可渲染一帧到 PNG，用于本地与 CI 视觉检查。

- 真实 LSP：`lsp_core` 客户端通过 stdio 管道驱动语言服务器（分帧、自动应答服务端请求、有序关闭），
  Windows 侧接入补全、诊断（行内标记与状态栏）、F12 跳转定义与 Ctrl+I 悬停。
- 文本缓冲升级为 piece table（`text_buffer`）：字符边界安全的插入/删除/替换、分组编辑、
  按编辑的撤销/重做历史；打开、保存、格式化刷新、剪切与 LSP 同步都使用同一份文本。
- 编辑器按可见窗口绘制（`Viewport` + `line_window`）：只取出并绘制屏幕上的行，新增编辑器滚动条，
  滚轮在代码列滚动（Shift 为水平滚动），光标始终保持在视野内。

- LSP 补齐：Ctrl+R 引用列表（再按一次依次跳转）、F2 重命名（编辑应用到打开缓冲与其他文件并刷新索引）、
  状态栏显示光标所在行的诊断。
- 调试：新增 `dap_core` 真实 DAP 会话（debugpy/lldb-dap/dlv），F9 断点、F5 启动/继续、Shift+F5 停止、
  F10 单步跳过、F11/Shift+F11 步入步出，停止后自动读取调用栈、作用域与变量并跳到停下的帧。
- AI：新增 Ctrl+Shift+A 的 AI 面板，回答由 `ai_core::ChatSession` 以子进程 curl 流式读入（Key 只走环境变量，
  Esc 可取消并保留已收到的部分回答），错误按 HTTP 状态给出可读说明。

- 增量项目索引（`index_core`）：按文件指纹只重解析变化的文件、复用未变化的部分、丢弃已删除的文件，
  编辑时原地更新当前文件；大项目不再为一次按键重扫整个项目。
- 项目树新增滚动条；设置、打包与树行在鼠标悬停时高亮；设置与扩展面板补齐完整中文文案。
- Linux 后端新增会话恢复（项目、打开的文件、最近项目）并读取运行时缓存清单，扩展面板显示缓存条目与占用空间。

- 共享语法扫描（`syntax_core`）：屏蔽注释与字符串（含反引号字符串）、按出现顺序配对括号与缩进块、
  把声明归属到它打开的作用域、记录调用所属作用域，并提供声明/引用角色；关系图的类范围、类成员与
  「被调用者」改为取自扫描，不再使用括号计数与词扫描启发式。
- `editor_core` 能识别无关键字的定义（`start() {`）而不会把 `if let Some(x) {` 或普通调用误判为定义。
- AI 面板按最近 12 轮携带多轮上下文，请求体由 `ai_core::chat_body_with_history` 构建（有上限）。

- 新增 `Ctrl+Shift+D` 问题与调试面板：语言服务器诊断、调试停止原因、调用栈、作用域变量与已设断点
  在同一列表中，方向键选择、回车跳转到对应文件与行。
- AI 会话按项目保存到 `.tc-ide-ai/history.conf`，打开项目时恢复；请求按最近 12 轮携带上下文。

- 语言工具状态改为真实探针：按语言列出语言服务器、格式化器与调试适配器，并报告每个是否真的可执行
  （Windows 扩展页与 Linux 扩展面板都会显示 `LSP/格式化/调试` 的 ok 或 missing）。
- 问题面板可用 Tab 在「全部 / 警告及以上 / 仅错误」之间过滤，行槽标记同步跟随。

- 共享动效模型（`motion_core`）：阻尼弹簧、收敛判定、浮层与抽屉两套手感参数、缓动与悬停逼近，
  两个后端统一使用，动画手感不再因平台而异。
- 新增平台边界守卫测试：共享核心不得出现 Win32/X11/Cocoa 类型、句柄、原始 FFI 与 `unsafe`，
  并检查共享模块都在 `main.rs` 中声明、平台模块仍在 `cfg(target_os)` 之后。

### 工程与 CI

- 新增共享的行位置弹簧 `motion_core::RowShift`：折叠文件夹时，树里保留下来的行滑到新位置，
  展开时再滑回来，不再是“箭头一转、下面的行瞬间跳上去”；切换项目会清空弹簧。

- 打包命令与打包目标下沉为共享纯函数：`workspace_core::PackageRequest` / `package_command`
  负责 Rust、Go、C、C++、Python、Java 的构建步骤以及 ZIP、`MakeAppx`、`signtool` 签名与结束/失败标记，
  `parse_package_targets` / `serialize_package_targets` 统一了两个后端写入 `.tc-ide-package-targets`
  的格式，`msix_identity` / `msix_display_name` / `msix_manifest` / `MSIX_ASSET_FILES`
  负责 MSIX 清单与图标槽位，清单版本随 `CARGO_PKG_VERSION` 变化（此前写死 0.2.0.0），
  设置与会话文件名也改为共享常量。两个后端都改为调用共享实现。
- Windows 后端按区域拆分为模块：`windows_app.rs` 从 7131 行的单文件降为 1359 行的模块根
  （Win32 声明、常量、应用状态与生命周期），其余拆到 `src/windows_app/` 下的 14 个模块——
  窗口过程与命令分发（`events`）、键盘（`keys`）、鼠标（`pointer`）、编辑器（`editor`）、
  工作区（`workspace`）、语言服务（`language`）、扩展（`extensions`）、打包（`packaging`）、
  调试（`debug`）、AI（`ai`）、底部终端面板（`panel`）、弹出终端（`detached`）、场景（`scene`）、
  动效（`motion`）。行为不变：拆分由逐行比对与 Windows 目标类型检查核对。
- 渲染器按场景区域拆分：`windows_renderer.rs` 中 2651 行的 `render` 改为共享 `ScenePaint`
  （画笔与几何）加 8 个区域绘制方法（终端面板、打包进度、打包浮层、设置、扩展、问题、AI、新建对话框），
  `render` 现在是一串可读的区域调用。
- 新增 `tests/module_layout.rs`：守卫这次拆分——区域模块必须存在且各带模块注释、
  单文件不得超过 1000/1500 行、每个场景区域都要有独立绘制方法并在 `render` 中被调用。
- CI 新增 `cargo fmt --check`、Linux 与 Windows 目标的 Clippy（`-D warnings`）、
  Windows 目标的类型检查，以及在 Xvfb 中渲染一帧的冒烟测试。
- 发布工作流（仅手动触发）在各自系统上生成产物：Linux 生成 `.tar.gz` 与真实 `.deb`
  （并尝试 AppImage），macOS 生成 `.app` 与 `.dmg`，Windows 生成 ZIP 与 MSIX。
  私钥只用于用户自己选择的证书，不会生成或上传。

### 尚未完成

见 `docs/KNOWN-ISSUES.md` 与 `docs/HANDOFF.md` 第 5 节：Windows 实机验收、语义分析的完整语法树、
macOS 原生后端、偏好设置与打包命令的共享模型下沉、控件按下/禁用/焦点状态、
文件树行位置弹簧与背景图裁剪框等仍未完成。
