# DeepSeek 交接：把 TC-IDE 核心体验做成 VS Code 扩展

> 本文是可直接执行的工程任务书。目标不是修改 TC-IDE 原生应用，也不是把原生窗口嵌入 VS Code，而是在本仓库新增一个独立、可安装、可调试、可打包为 `.vsix` 的 VS Code 扩展。

## 1. 最终目标

扩展暂定名称：**TC-IDE Companion**  
建议扩展 ID：`tianchuangya.tc-ide-companion`

用户在 VS Code 中安装本扩展后，应能获得 TC-IDE 最有辨识度的能力：

1. 项目、文件、Class、函数、变量之间的语义关系图；
2. 跟随当前文件、光标符号和鼠标焦点的调用者/被调用关系；
3. 编辑器中的定义、调用、引用、调试值等语义提示；
4. `.tcIDE` 项目描述文件，可保存关系图、构建、打包和界面状态；
5. Build、Run、Debug、Output、Terminal 的统一任务入口；
6. TC-IDE 风格的深色/浅色主题、低色彩噪声界面和流畅过渡；
7. 黑洞吸入、白洞吐出式打包可视化，并连接真实构建任务；
8. 中文为默认显示语言，同时提供英文本地化。

扩展必须使用 VS Code 正式公开 API。禁止通过修改 VS Code 安装目录、注入 DOM、Monkey Patch 内部模块或依赖未公开私有 API 来实现效果。

## 2. 代码语言原则

源码语言不作硬性限制，可以使用 TypeScript、JavaScript、Rust、C++、WASM 或其他语言组合，但必须遵守以下边界：

- VS Code 桌面扩展的入口最终必须编译成 Extension Host 可加载的 JavaScript；推荐 TypeScript。
- Rust/C++ 适合实现大型项目索引器、解析器或独立辅助进程，不能直接代替扩展入口。
- 可移植的纯计算模块可以编译成 WASM，在 Extension Host 或 Webview 中运行。
- 第三方原生程序必须按 Windows、Linux、macOS 分别打包、校验和启动，不能只在开发者当前系统可用。
- 第一版优先采用 **TypeScript 扩展入口 + VS Code Language API + Webview View**。只有实际性能数据证明 TypeScript 索引不足时，再增加 Rust sidecar。

不接受“为了显得性能高”而提前引入复杂的多语言通信。每增加一种运行时，都必须写明它解决的真实性能问题和失败回退方案。

## 3. 与原生 TC-IDE 的关系

原生应用与 VS Code 扩展是两个产品外壳，不能让其中一个覆盖另一个：

- 原生 TC-IDE 保留 Rust 原生窗口、三平台后端和自己的扩展生态路线；
- VS Code 扩展复用 TC-IDE 的产品概念、视觉语言和项目文件格式；
- 可复用纯逻辑时，优先定义稳定的数据协议，而不是复制一份以后无法同步的算法；
- 不允许为了扩展而破坏 `src/` 下现有原生程序；
- 新扩展全部放在仓库根目录的 `vscode-extension/` 中。

开始前必须阅读：

- `docs/HANDOFF.md`
- `docs/ARCHITECTURE.md`
- `docs/VSCODE-COMPATIBILITY.md`
- `src/graph_core.rs`
- `src/syntax_core.rs`
- `src/tcide_project.rs`
- `src/theme_core.rs`
- `src/package_motion.rs`

## 4. VS Code 能力映射

| TC-IDE 能力 | VS Code 中的实现方式 | 说明 |
|---|---|---|
| 右侧调用层级 | `WebviewViewProvider` | 用 Canvas 或 SVG 绘制，不使用大量绝对定位 DOM 节点 |
| 项目/文件/Class/函数层级 | Language API + LSP + 文档符号 | 优先请求 `vscode.executeDocumentSymbolProvider`、定义、引用与调用层级 Provider |
| 当前符号跟随 | `window.onDidChangeTextEditorSelection`、可见编辑器事件 | 防抖、可取消，不能每次光标移动扫描整个项目 |
| 编辑器内语义提示 | Decorations、CodeLens、InlayHints、Hover | VS Code 编辑器不能绘制任意跨区域连线，不得假装能完整复刻原生轨道 |
| 定义/调用/引用框选 | `TextEditorDecorationType` | 不永久污染编辑区；光标离开或关闭功能后必须清理 |
| 调试变量显示 | Debug API + InlayHints/Decorations | 仅在暂停状态读取并显示，继续运行立即清除旧值 |
| 项目状态 | `.tcIDE` + `workspaceState` | 可提交的项目配置与本机 UI 状态分开保存 |
| Build/Run | `TaskProvider`、`tasks.executeTask` | 必须连接真实任务与 Problems Matcher |
| Debug | `DebugConfigurationProvider`、`debug.startDebugging` | 复用用户已有调试器，不伪造调试输出 |
| 终端 | `window.createTerminal` | 使用真实 VS Code Terminal，不在 Webview 伪造终端 |
| 打包动画 | Webview Canvas/SVG | 动画只表现真实任务进度；失败时必须停在失败态并展示日志 |
| 主题 | `contributes.themes` | 至少提供 TC-IDE Dark 与 TC-IDE Light |
| 设置 | `contributes.configuration` | 所有开关、动画强度、索引排除项和隐私选项集中管理 |
| 命令与菜单 | `contributes.commands`、`menus`、`keybindings` | 每个可见按钮必须连接真实命令 |

### 4.1 无法一比一复刻的部分

VS Code 扩展不能接管顶部菜单栏、标签栏、原生编辑器布局或在编辑器与侧栏之间直接绘制跨区域箭头。遇到这些限制时：

1. 使用原生 Decorations、CodeLens、InlayHints 表达代码侧关系；
2. 使用右侧 Webview 图表达完整路径；
3. 两者通过选中节点、Reveal Range 和命令联动；
4. 文档中明确写成“适配设计”，禁止用截图或静态元素伪装成已实现功能。

## 5. 建议目录结构

```text
vscode-extension/
├── package.json
├── package-lock.json
├── tsconfig.json
├── esbuild.mjs
├── README.md
├── CHANGELOG.md
├── LICENSE
├── src/
│   ├── extension.ts              # activate/deactivate，仅做装配
│   ├── commands.ts               # 命令注册
│   ├── config.ts                 # 配置与 Workspace Trust
│   ├── project/
│   │   ├── tcideFile.ts          # .tcIDE 读写与迁移
│   │   └── workspaceModel.ts
│   ├── semantic/
│   │   ├── indexer.ts
│   │   ├── providers.ts
│   │   ├── graphModel.ts
│   │   └── cancellation.ts
│   ├── editor/
│   │   ├── decorations.ts
│   │   ├── codeLens.ts
│   │   └── inlayHints.ts
│   ├── graph/
│   │   ├── graphViewProvider.ts
│   │   └── protocol.ts
│   ├── tasks/
│   │   ├── taskProvider.ts
│   │   ├── buildRun.ts
│   │   └── packaging.ts
│   ├── debug/
│   │   └── debugBridge.ts
│   └── test/
├── media/
│   ├── graph.js
│   ├── graph.css
│   ├── packaging.js
│   └── icons/
├── themes/
│   ├── tc-ide-dark-color-theme.json
│   └── tc-ide-light-color-theme.json
├── l10n/
│   ├── bundle.l10n.json
│   └── bundle.l10n.zh-cn.json
└── scripts/
    └── verify-package.mjs
```

如果使用 Rust sidecar，新增 `vscode-extension/native/`，输出文件必须进入平台专属目录，且扩展在 sidecar 缺失或启动失败时仍能降级到 VS Code Provider 模式。

## 6. 数据与消息协议

### 6.1 `.tcIDE` 文件

不得把所有本机状态写进项目文件。建议格式：

```json
{
  "schemaVersion": 1,
  "project": {
    "name": "demo",
    "root": "."
  },
  "semanticGraph": {
    "defaultScope": "auto",
    "relation": "follow",
    "excluded": ["**/node_modules/**", "**/target/**", "**/.git/**"]
  },
  "tasks": {
    "build": "build",
    "run": "run",
    "package": "package"
  },
  "packageTargets": []
}
```

窗口宽高、当前滚动位置、最近选中节点等本机状态放入 `workspaceState`；密钥放入 `SecretStorage`；禁止写入 `.tcIDE`。

### 6.2 Webview 协议

Extension Host 与 Webview 的消息必须是带版本和类型的联合结构，例如：

```ts
type HostToGraph =
  | { version: 1; type: 'replaceGraph'; requestId: string; graph: GraphSnapshot }
  | { version: 1; type: 'focusNode'; nodeId: string }
  | { version: 1; type: 'setTheme'; theme: ThemeTokens }
  | { version: 1; type: 'packageProgress'; progress: PackageProgress };

type GraphToHost =
  | { version: 1; type: 'ready' }
  | { version: 1; type: 'openLocation'; uri: string; line: number; column: number }
  | { version: 1; type: 'changeScope'; scope: GraphScope }
  | { version: 1; type: 'cancel'; requestId: string };
```

不得传递函数、任意脚本或未经验证的文件路径。所有输入都要做结构和工作区边界校验。

## 7. 视觉与动效要求

### 7.1 整体风格

- 深浅两套主题必须同时完成；
- 低色彩噪声，以中性色层级加单一冷色强调为主；
- 不堆砌霓虹、渐变、发光和无意义粒子；
- 动画用于说明焦点、来源、去向和状态变化；
- 尊重 `prefers-reduced-motion` 和 VS Code 减少动效设置；
- 所有颜色使用 VS Code Theme Tokens 或 CSS 变量，不能写死只适合暗色主题的颜色。

### 7.2 关系图

- 顶部范围选择使用一条可移动的语义轨道，不排列一排笨重按钮；
- 范围：自动、整个项目、当前文件夹、当前文件、当前符号；
- 关系：跟随、调用者、被调用；
- 指示线移动必须使用弹簧或缓动过渡，不能瞬移；
- 节点按 `项目 → 文件夹 → 文件 → Class → 函数/变量` 显示；
- 节点多时必须聚类、虚拟化或只渲染可见区域；
- 滚轮只改变视口或焦点，不得触发整个项目反复重建；
- 点击节点必须打开真实文件并定位行列。

### 7.3 编辑器语义反馈

- 当前符号用细边框或底色标识；
- 定义、调用、引用采用不同但克制的颜色与图标；
- 使用 Decorations/CodeLens/InlayHints，禁止覆盖正文导致代码不可读；
- 注释不需要重复画“指向注释本身”的箭头；
- 调试变量值显示在对应变量附近，并在 Debug 继续后清除；
- 快速输入时暂停昂贵动画，优先保证编辑器响应。

### 7.4 打包动画

- 源文件、函数、变量以抽象线条或极简图标向手绘黑洞汇聚；
- 黑洞由多圈黑色线条顺时针旋转表示；
- 靠近中心时加速并产生拉伸，不绘制多余关系连线；
- 构建成功后切换为反向旋转的白洞并吐出目标包；
- 动画时长由真实任务阶段驱动，不能用固定定时器假装完成；
- 失败时停止动画，展示失败阶段、退出码和“打开日志”；
- 成功后提供“打开输出目录”。

## 8. 实施阶段

### 阶段 A：可运行骨架

- [ ] 创建 `vscode-extension/` 独立 npm 工程；
- [ ] `F5` 启动 Extension Development Host；
- [ ] 注册 Activity Bar 容器、关系图视图、命令和设置；
- [ ] 完成中英文本地化；
- [ ] 加入 TC-IDE Dark/Light 主题；
- [ ] 建立单元测试、扩展集成测试、Lint、Type Check、打包检查；
- [ ] CI 只构建和测试 `.vsix`，未经项目所有者明确授权不得创建 GitHub Release。

### 阶段 B：真实语义关系

- [ ] 从 VS Code DocumentSymbol、Definition、References、Call Hierarchy API 获取真实数据；
- [ ] 构建稳定 node ID，不使用易变数组下标；
- [ ] 实现范围、搜索、调用者/被调用切换；
- [ ] 实现增量缓存、取消、排除目录与大项目限流；
- [ ] Webview 图与编辑器 Reveal/Selection 双向联动；
- [ ] 无语言 Provider 时显示明确降级说明，不制造关系。

### 阶段 C：编辑器增强

- [ ] 当前符号、定义、调用、引用 Decorations；
- [ ] CodeLens 显示引用数量和打开关系图命令；
- [ ] 可配置的 InlayHints；
- [ ] Debug 暂停状态的变量值提示；
- [ ] 所有 Decoration 在文档关闭、配置关闭和插件停用时释放。

### 阶段 D：项目与任务

- [ ] `.tcIDE` 文件创建、读取、迁移、错误提示；
- [ ] TaskProvider 提供 Build、Run、Package；
- [ ] 真实 Terminal/Output/Problems 联动；
- [ ] C/C++、Rust、Python、Java、Go、JS/TS 的默认任务探测；
- [ ] 命令缺失、退出非零、路径冲突必须给出可操作错误。

### 阶段 E：打包可视化

- [ ] Canvas/SVG 动画与真实 TaskExecution 状态绑定；
- [ ] 多目标选择按项目保存，首次默认不全选；
- [ ] 成功打开输出目录，失败打开日志；
- [ ] 减少动效模式下使用淡入淡出替代旋转和拉伸；
- [ ] 动画不可阻塞 Extension Host。

### 阶段 F：性能、安全与交付

- [ ] Webview 使用严格 CSP 和 nonce；禁止任意远程脚本；
- [ ] 尊重 Workspace Trust，未信任工作区不执行任务或 sidecar；
- [ ] 文件扫描遵守 `files.exclude`、`search.exclude`、`.gitignore` 和扩展自己的排除项；
- [ ] Extension Host 中禁止同步遍历大型仓库；
- [ ] 输出激活耗时、索引耗时、节点数和取消原因到专属 Output Channel；
- [ ] 生成可本地安装的 `.vsix`；
- [ ] README 写清功能、限制、隐私、设置、快捷键和故障排查。

## 9. 性能底线

- 扩展激活阶段只注册 Provider 和命令，不立即全项目扫描；
- 索引按需开始，支持 `CancellationToken`；
- 光标与文本事件必须防抖，旧请求返回后不得覆盖新焦点；
- Webview 只接收增量或有界快照，禁止每帧传完整项目；
- 图节点超出阈值时聚类或分页；阈值必须可配置；
- Canvas 动画与 Extension Host 计算分离；
- 关闭关系图、关闭动画或停用扩展后 CPU 应回到空闲状态；
- sidecar 崩溃不能带崩 VS Code，且必须有可见错误和重启入口。

## 10. 测试要求

至少包含以下自动化测试：

1. `.tcIDE` 正常、缺字段、未知字段和版本迁移；
2. Windows 路径、POSIX 路径、空格、中文和引号；
3. 关系节点 ID 稳定性、去重、循环调用和跨文件跳转；
4. 光标快速移动时旧请求被取消；
5. 文档关闭后 Decoration 被清理；
6. Build/Run/Package 成功、失败、取消和命令缺失；
7. 未信任工作区不会执行进程；
8. Webview 消息拒绝未知类型、非法 URI 和工作区外路径；
9. 深色、浅色、高对比度和减少动效；
10. 至少在 Windows、Ubuntu、macOS CI 完成 Type Check、测试和 VSIX 打包。

建议同时准备 `fixtures/`：C、C++、Rust、Python、Java 各一个包含跨文件调用、类、变量、递归和同名符号的小项目。

## 11. 完成定义

只有同时满足以下条件，才能说“这一阶段完成”：

- 功能连接真实 VS Code API 或真实任务，不是静态演示；
- 所有可见按钮都有行为、错误反馈和禁用状态；
- 重新加载 VS Code 后项目设置和必要视图状态恢复；
- 深色与浅色主题都可读；
- 大项目不会因光标移动持续全量扫描；
- 扩展停用后没有残留进程、终端监听器或 Decoration；
- 自动化测试通过；
- 可以通过 `code --install-extension <file>.vsix` 本地安装；
- README 明确列出暂不支持的能力；
- 没有创建版本标签或 GitHub Release，除非项目所有者明确说“正式开始发布”。

## 12. 明确禁止的伪实现

- 禁止用假终端文本代替 `Terminal` 或 `Task`；
- 禁止 Build/Run 只打开终端却不执行命令；
- 禁止用固定延时把打包动画标记为成功；
- 禁止把随机或正则猜测结果冒充 LSP 精确关系；
- 禁止静态截图冒充关系图；
- 禁止按钮点击没有反馈；
- 禁止 Webview 无限轮询 Extension Host；
- 禁止把 API Key、绝对本机路径或用户源码写入日志；
- 禁止宣称“完全复刻 TC-IDE”或“兼容所有语言”，除非对应验收已经通过。

## 13. DeepSeek 工作方式与交接格式

DeepSeek 每个阶段都必须：

1. 先查看当前 Git 状态，保留已有改动；
2. 完成一组能独立验收的功能；
3. 运行 Type Check、测试、Lint 和 VSIX 打包检查；
4. 在本文件末尾追加“实际完成记录”，写明文件、命令、测试结果和未完成事项；
5. 只提交源码，不创建 Release；
6. 遇到 VS Code API 限制时记录证据并采用适配设计，不伪造能力；
7. 不因切换到下一项而丢弃尚未完成的旧任务。

交接记录模板：

```md
### YYYY-MM-DD / 阶段 X

- 完成：
- 修改文件：
- 验证命令：
- 验证结果：
- 截图或录像：
- 未完成：
- 下一步：
```

## 14. 官方参考

- Extension API：<https://code.visualstudio.com/api>
- Extension Anatomy：<https://code.visualstudio.com/api/get-started/extension-anatomy>
- Extension Manifest：<https://code.visualstudio.com/api/references/extension-manifest>
- Contribution Points：<https://code.visualstudio.com/api/references/contribution-points>
- Webview：<https://code.visualstudio.com/api/extension-guides/webview>
- Tree View：<https://code.visualstudio.com/api/extension-guides/tree-view>
- Task Provider：<https://code.visualstudio.com/api/extension-guides/task-provider>
- Testing Extensions：<https://code.visualstudio.com/api/working-with-extensions/testing-extension>
- Publishing：<https://code.visualstudio.com/api/working-with-extensions/publishing-extension>

实现时优先使用这些官方文档，不依据博客里访问 VS Code 内部 DOM 的做法。

## 15. 实际完成记录

尚未开始。第一步应创建 `vscode-extension/` 骨架并完成阶段 A，不得先制作静态 Webview 演示来冒充完整扩展。
