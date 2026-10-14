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

### Linux 后端

- 关系图不再使用演示数据：改为扫描项目目录（没有项目时使用已打开文件）建立真实索引，
  由 `graph_core` 提供范围、搜索、折叠与关系类型。
- 偏好设置读取改走共享解析器。
- 新增 `--render-frame <path>`：无需映射窗口即可渲染一帧到 PNG，用于本地与 CI 视觉检查。

### 工程与 CI

- CI 新增 `cargo fmt --check`、Linux 与 Windows 目标的 Clippy（`-D warnings`）、
  Windows 目标的类型检查，以及在 Xvfb 中渲染一帧的冒烟测试。
- 发布工作流（仅手动触发）在各自系统上生成产物：Linux 生成 `.tar.gz` 与真实 `.deb`
  （并尝试 AppImage），macOS 生成 `.app` 与 `.dmg`，Windows 生成 ZIP 与 MSIX。
  私钥只用于用户自己选择的证书，不会生成或上传。

### 尚未完成

见 `docs/KNOWN-ISSUES.md` 与 `docs/HANDOFF.md` 第 5 节：Windows 实机验收、文本缓冲升级、
LSP/DAP 传输层接入、Tree-sitter、macOS 原生后端、`windows_app.rs` 拆分、大项目后台增量索引、
控件状态与完整中文文案等仍未完成。
