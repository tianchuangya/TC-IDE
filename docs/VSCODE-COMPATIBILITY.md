# TC-IDE 与 VS Code：差距、取舍与扩展兼容路线

## 结论

TC-IDE 不应复刻另一个 VS Code。它应保留原生 Rust 界面、语义轨道、代码与项目关系可视化、动效和打包体验，同时补齐可靠性、工程工具和扩展生态。

“兼容 VS Code 扩展”不能等同于能读取 `.vsix`。真正的兼容至少包含：解析清单、Contribution Points、扩展激活、`vscode` API、Node/Web 扩展宿主、Webview、权限与崩溃隔离。TC-IDE 当前已有自己的扩展目录和 LSP/DAP/终端能力，但尚无 VS Code Extension Host，因此不能宣称任意 VS Code 插件可运行。

首选来源是 Open VSX 与用户本地导入的 `.vsix`。在确认分发授权以前，不把 Microsoft Visual Studio Marketplace 当作 TC-IDE 的公共插件源。

## 能力对比

| 领域 | TC-IDE 当前优势 | 相对 VS Code 的主要差距 | 优先级 |
|---|---|---|---|
| 视觉与交互 | 原生绘制、语义轨道、关系图、打包动效，可形成独有体验 | 动效与布局仍需持续做性能和可访问性验收 | P1 |
| 编辑可靠性 | 已有真实文本缓冲、撤销重做、补全、诊断、跳转、重命名 | 大文件、超大仓库、多光标、列选择、复杂重构与崩溃恢复不够成熟 | P0 |
| 语言能力 | 已接 LSP、DAP、格式化和语法着色 | 语言覆盖、语法树精度、内联提示、Code Action、语义 Token 等仍弱 | P0 |
| 工程工作流 | Build/Run/终端、项目模板、打包入口已形成一体化 | Git/合并、测试资源管理器、任务/问题体系、远程开发、容器与 Notebook 缺口大 | P0/P1 |
| 扩展生态 | 有下载、校验、缓存、卸载和项目插件基础 | 没有 VSIX 导入、Open VSX 客户端、扩展宿主和 `vscode` API 兼容层 | P0 |
| 跨平台 | Linux/Windows/macOS 都有原生后端 | Windows/macOS 真机矩阵、IME、DPI、无障碍、签名安装包仍需系统验收 | P0 |
| 性能与安全 | 可控制原生渲染管线，核心不依赖网页运行时 | 缺少插件进程隔离、权限声明、资源限额、卡顿与崩溃归因 | P0 |

## 扩展兼容分级

扩展页必须显示真实兼容等级，不能只显示“已安装”。

1. **声明式资源**：主题、图标主题、TextMate grammar、snippet、language configuration。无需运行第三方代码，优先实现，目标是高兼容。
2. **协议适配器**：主要贡献语言服务器、调试适配器或任务的扩展。把它们映射到 TC-IDE 已有 LSP、DAP 与终端内核，目标是中高兼容。
3. **VS Code API 宿主**：带 `main` 或 `browser` 入口的普通扩展。需要独立 Node/WebWorker 进程、RPC 桥和一部分 `vscode` API，按常用 API 逐批兼容。
4. **高级工作台扩展**：Webview、Custom Editor、Notebook、SCM、Remote、认证、私有或 proposed API。成本最高，初期明确标成部分兼容或不支持。

本轮已经在 `extension_core` 增加 VS Code `package.json` 的安全检查器。它不会执行插件代码，会提取发布者、版本、引擎范围、入口和 Contribution Points，并把插件归入上述运行时类别。它是 VSIX/Open VSX 导入界面的真实能力判断基础，不是“已经兼容全部插件”的标志。

## 实施顺序

### P0：先让它成为可靠的日常 IDE

- 会话与未保存内容崩溃恢复；大项目、长行、大文件基准和卡顿追踪。
- 完整 Git 状态、差异、暂存、提交、冲突解决。
- LSP Code Action、语义 Token、签名帮助、内联提示、工作区符号。
- 测试发现/运行/失败跳转；统一 Problems、Output、Run、Debug 状态。
- Windows/macOS/Linux 真机回归矩阵，尤其是 DPI、IME、剪贴板、终端与文件对话框。

### P0：扩展桥第一阶段

- 本地 `.vsix` 导入、签名/摘要验证、解包与卸载。
- Open VSX 搜索、版本选择、更新和依赖解析。
- 读取 `package.json` 并在安装前显示兼容等级、需要的权限与缺失 API。
- 转换主题、grammar、snippet 与 language configuration；失败时指出具体字段。

### P1：协议与 Extension Host

- 把 debugger、task 和 language-server 贡献映射到已有 DAP/LSP/终端。
- 单独的 Node Extension Host 进程；每个插件有启动超时、内存/CPU 监控和崩溃隔离。
- Rust 主程序与宿主用版本化 JSON-RPC 通信。
- 首批 `vscode` API：`commands`、`workspace`、`window`、`languages`、`tasks`、`debug`、`configuration`、文档/编辑器事件。
- 兼容性测试仓库固定一组常用开源扩展，逐版本记录通过能力，而不是用一个“兼容”开关概括。

### P2：高级生态

- 原生视图容器与 Tree View 映射。
- 可选的沙箱 Webview；如果坚持完全无 Webview，则明确放弃依赖 HTML 工作台的插件。
- Notebook、SCM Provider、Authentication、Remote 等逐类设计，不能让插件直接接管 TC-IDE 主进程。

## 验收标准

- 安装前能解释插件为何可用、部分可用或不可用。
- 插件崩溃不会带崩编辑器；禁用后不再启动、不残留进程。
- 主题/grammar/snippet 的导入有视觉或行为回归测试。
- API 兼容以自动化契约测试为准；未实现的 API 返回明确错误并记录来源插件。
- 插件来源、版本、摘要、权限、更新与数据目录可被用户查看和清理。

## 不应做的事

- 不直接抓取 Microsoft Marketplace 并重新分发未确认授权的扩展。
- 不在 TC-IDE 主进程内 `eval` 第三方 JavaScript。
- 不把“成功解压 VSIX”显示成“插件可运行”。
- 不为追求插件数量破坏原生编辑器的帧率、启动速度和语义交互。
