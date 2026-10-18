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

## 15. 无视觉模型执行规则

本节假设实现者完全看不见界面，不能凭“好看”“差不多”“有科技感”之类主观词工作。实现者只能依据数字、状态和自动化结果施工。

### 15.1 唯一判断顺序

出现冲突时按以下顺序决定，不得自行发挥：

1. VS Code 正式 API 和安全限制；
2. 本文的坐标、尺寸、颜色、状态与动画参数；
3. `src/theme_core.rs`、`src/motion_core.rs`、`src/graph_core.rs`、`src/package_motion.rs` 的共享常量；
4. 经项目所有者或 Codex 明确批准的基准截图；
5. 文字需求；
6. 实现者自己的审美最后考虑。

“看起来差不多”不是验收证据。必须用 DOM 几何断言、状态测试、截图差异和帧性能数据证明。

### 15.2 不准由实现者批准自己的视觉基准

- DeepSeek 可以实现页面、运行截图脚本、输出待验收截图；
- DeepSeek 不能把自己第一次生成的截图直接保存为“正确基准”并宣布通过；
- 首批基准必须由项目所有者或具备视觉能力的 Codex 查看后批准；
- 基准批准后，后续改动才允许用自动截图回归；
- 如果没有已批准基准，就报告“等待视觉验收”，不能报告“视觉一致”。

### 15.3 禁止模糊实现

下面这些提交说明一律视为未完成：

- “大概复刻”；
- “类似苹果风格”；
- “做了一个炫酷界面”；
- “应该和截图差不多”；
- “动画已经很丝滑”；
- “肉眼看不出区别”。

必须改成可验证描述，例如：“正常宽度下卡片左边为 42px，深度每增加一级右移 16px；自动化几何测试误差不超过 1px”。

## 16. VS Code 中的空间方向与区域

### 16.1 用户面对屏幕时的方向

- **左**：VS Code Activity Bar 与资源管理器一侧；
- **中**：VS Code 原生代码编辑器；
- **右**：Secondary Side Bar，推荐放 TC-IDE 关系图；
- **下**：VS Code 原生 Panel，显示 Terminal、Problems、Output、Debug Console；
- **代码上方**：VS Code 原生标签栏与编辑器工具栏，扩展不能接管；
- **最下方**：VS Code Status Bar，扩展只允许增加自己的状态项。

推荐用户布局如下：

```text
┌──────────────────────────── VS Code 原生标题/命令中心 ────────────────────────────┐
├───┬────────────────────┬──────────────────────────────────┬──────────────────────┤
│ A │ 左侧资源管理器      │ 中央代码编辑器                    │ TC-IDE 语义关系图      │
│ c │                    │                                  │ 标题 + 操作            │
│ t │ 搜索框              │  代码 + Decoration + CodeLens   │ 范围语义轨道            │
│ i │ 项目文件树          │  Inlay Hint / 调试变量           │ 搜索框                  │
│ v │                    │                                  │ 路径 + 层级卡片          │
│ i │                    │                                  │ 状态摘要                │
│ t │                    │                                  │                        │
│ y │                    │                                  │                        │
├───┴────────────────────┴──────────────────────────────────┴──────────────────────┤
│ VS Code 原生 Panel：TERMINAL / PROBLEMS / OUTPUT / DEBUG CONSOLE                  │
├───────────────────────────────────────────────────────────────────────────────────┤
│ VS Code Status Bar：TC-IDE 索引状态 · 当前范围 · Build/Run 状态                    │
└───────────────────────────────────────────────────────────────────────────────────┘
```

`A c t i v i t y` 表示最左侧竖直 Activity Bar，不是需要真的逐字竖排。

### 16.2 VS Code 限制下的位置策略

扩展可以注册 View Container，但不能强制用户把它移动到 Secondary Side Bar。因此：

1. 第一次激活时打开 TC-IDE View Container；
2. README 用中文图示教用户右键视图标题并移动到 Secondary Side Bar；
3. 扩展命令“TC-IDE：打开语义关系图”只负责 Reveal View；
4. 不调用私有命令强制移动视图；
5. View 在左侧时必须同样可用，不能写死“右侧才绘制”；
6. 宽度变化立即重排，不依赖窗口总宽度。

### 16.3 宽度模式

Webview 只根据自身 `clientWidth` 选择模式：

| 模式 | 宽度 | 行为 |
|---|---:|---|
| 极窄 | `< 220px` | 只保留标题、当前关系、搜索图标和节点主标题；隐藏路径详情 |
| 紧凑 | `220–309px` | 单行标题、缩短元数据；范围轨道可水平滚动 |
| 舒适 | `310–429px` | 显示节点副标题和短路径，路径最多 16 个字符后加 `…` |
| 展开 | `≥ 430px` | 显示完整副标题、相对路径与行号 |

任何模式都不能出现水平页面滚动条。只有范围轨道内部允许横向滚动。

## 17. 设计 Token：不得改成近似值

### 17.1 尺寸 Token

所有数值为 CSS logical pixel；浏览器缩放和系统 DPI 交给 VS Code/Chromium 处理，不手动乘 DPR。

| 名称 | 数值 | 用途 |
|---|---:|---|
| `space-1` | 4px | 图标与短文本之间 |
| `space-2` | 8px | 控件内边距、小间距 |
| `space-3` | 12px | 卡片内部与紧凑区间距 |
| `space-4` | 16px | 浮层边距、区域标准间距 |
| `space-6` | 24px | 大区域分隔 |
| `space-8` | 32px | 标准控件高度 |
| `top-bar-height` | 48px | 仅用于独立预览壳；不能修改 VS Code 顶栏 |
| `menu-bar-height` | 30px | 仅用于独立预览壳 |
| `tab-strip-height` | 40px | 仅用于独立预览壳 |
| `status-bar-height` | 28px | 状态区参考高度 |
| `control-height` | 32px | 输入框、按钮、轨道触发区 |
| `control-radius` | 8px | 卡片、输入框、按钮圆角 |
| `panel-margin` | 16px | 大浮层外边距 |
| `semantic-rail` | 2px | 活动标签、范围、关系焦点线 |
| `graph-row-height` | 58px | 一行关系节点占位高度 |
| `graph-first-row` | 96px | 无搜索区时第一个节点相对内容顶部 |
| `graph-footer-height` | 34px | 图底部范围摘要 |
| `graph-depth-indent` | 16px | 每一级层级右移量 |
| `graph-path-x` | 30px | 纵向路径线距面板左边 |
| `graph-card-gap` | 12px | 路径线到一级卡片的间隔 |
| `graph-marker-inset` | 6px | 折叠箭头距卡片左边 |
| `graph-marker-width` | 12px | 折叠箭头占位宽度 |
| `graph-max-nodes` | 240 | 绝对安全上限，超过必须聚类 |

### 17.2 字体 Token

Webview 不下载远程字体。按以下顺序使用：

```css
--tc-ui-font: var(--vscode-font-family);
--tc-code-font: var(--vscode-editor-font-family);
```

| 内容 | 字体 | 字号 | 行高 | 字重 |
|---|---|---:|---:|---:|
| 面板标题 | UI | 14px | 20px | 600 |
| 控件正文 | UI | 13px | 18px | 400 |
| 节点主标题 | UI | 13px | 18px | 500 |
| 节点副标题 | UI | 11px | 16px | 400 |
| 路径/行号 | Code | 11px | 16px | 400 |
| 状态与空状态 | UI | 12px | 18px | 400 |

不使用全大写长标题。`TC-IDE` 品牌名保持原大小写。中文不能依赖手工增加字符间距。

### 17.3 暗色主题

以下颜色直接来自 `theme_core::Palette::dark()`：

| Token | HEX |
|---|---|
| background | `#0D1219` |
| panel | `#151D27` |
| raised | `#1D2937` |
| border | `#2B394A` |
| text | `#D7E2ED` |
| muted | `#91A0B2` |
| bright | `#EDF4FB` |
| accent | `#78BAFF` |
| selection | `#243D57` |
| selected text | `#EDF4FB` |
| keyword | `#64D2FF` |
| type | `#30D158` |
| function | `#BF5AF2` |
| constant | `#FFD60A` |
| variable | `#E8E8ED` |
| parameter | `#C9C9D1` |
| property | `#9FD7FF` |
| macro | `#FF8FA3` |
| module | `#B9B9C6` |
| preprocessor | `#FF375F` |
| literal | `#FF9F0A` |
| comment | `#8E8E93` |
| shadow | `#070B10` |

### 17.4 亮色主题

以下颜色直接来自 `theme_core::Palette::light()`：

| Token | HEX |
|---|---|
| background | `#F5F5F7` |
| panel | `#FFFFFF` |
| raised | `#F2F2F7` |
| border | `#D1D1D6` |
| text | `#3A3A3C` |
| muted | `#6E6E73` |
| bright | `#1D1D1F` |
| accent | `#007AFF` |
| selection | `#DBEAFE` |
| selected text | `#0A5AA8` |
| keyword | `#0B5394` |
| type | `#146B2E` |
| function | `#71317F` |
| constant | `#785700` |
| variable | `#24292F` |
| parameter | `#3D4451` |
| property | `#1B5E8A` |
| macro | `#8A2F45` |
| module | `#4A4F57` |
| preprocessor | `#9E1B3D` |
| literal | `#8A4B08` |
| comment | `#5F6368` |
| shadow | `#C7C7CC` |

### 17.5 VS Code 主题变量映射

Webview 首先使用 VS Code 变量适应用户主题，再用上述色板作为 TC-IDE 主题和测试基准：

```css
:root {
  --tc-bg: var(--vscode-sideBar-background, #0D1219);
  --tc-panel: var(--vscode-editorWidget-background, #151D27);
  --tc-raised: var(--vscode-input-background, #1D2937);
  --tc-border: var(--vscode-panel-border, #2B394A);
  --tc-text: var(--vscode-foreground, #D7E2ED);
  --tc-muted: var(--vscode-descriptionForeground, #91A0B2);
  --tc-accent: var(--vscode-focusBorder, #78BAFF);
  --tc-selection: var(--vscode-list-activeSelectionBackground, #243D57);
}
```

TC-IDE Dark/Light 主题文件必须把相应 VS Code keys 设置为上述色板；在其他用户主题下，Webview 跟随宿主变量，不强制覆盖整个 VS Code。

## 18. 组件逐像素施工图

### 18.1 关系图视图纵向顺序

Webview 根元素从上到下只能按以下顺序排列：

```text
y=0      12px 上边距
y=12     标题行，高 32px
y=44      8px 间距
y=52     关系轨道，高 32px
y=84      8px 间距
y=92     搜索框，高 32px
y=124     8px 间距
y=132    面包屑/当前焦点，高 24px
y=156     8px 间距
y=164    图滚动视口开始，延伸到 footer 上方
bottom   footer，高 34px
```

当搜索框折叠时，隐藏 `y=92..124` 的 32px，但保留一个 8px 间距；不得留下空白洞。

根区域左右内边距均为 12px。标题行和 footer 固定；只有图滚动视口滚动。

### 18.2 标题行

- 左侧：16×16 TC-IDE 图标；
- 图标右侧 8px：标题“调用层级”；
- 最右侧依次为搜索、固定/跟随、刷新、更多，图标命中框均为 28×28px；
- 图标之间 2px；
- 默认图标色为 muted，悬停为 text，按下为 bright；
- 刷新只重新请求当前范围，不能无条件重建全项目；
- 未实现的更多菜单项不显示，禁止画假按钮。

### 18.3 关系轨道

关系项严格为：`跟随`、`调用者`、`被调用`。

- 三项位于同一行，不画三个独立实心按钮；
- 文本左右内边距 10px，高 30px；
- 未选中文本使用 muted；选中文本使用 bright；
- 轨道底部 y=30px 处画 1px border；
- 选中项下方画 2px accent 指示线；
- 指示线从旧项移动到新项，不可销毁后在新位置瞬间重建；
- 点击后立即移动视觉焦点，同时发起可取消数据请求；
- 请求失败时保留选中项，并在内容区显示错误，不让轨道跳回。

### 18.4 范围选择

范围严格为：`自动`、`整个项目`、`当前文件夹`、`当前文件`、`当前符号`。

- 默认显示当前范围名称和一个向下 chevron；
- 展开后是浮层列表，不在狭窄面板中横排五个大按钮；
- 浮层距触发器 4px，圆角 8px，1px border；
- 每项高 32px，左右内边距 10px；
- 选中项左侧画 2px accent 短线，不能用字母 `V`；
- 自动范围依次偏好：鼠标悬停符号 → 光标符号 → 当前文件；
- 鼠标离开编辑器不应永久保留一个陈旧 hover，300ms 后回退到光标焦点。

### 18.5 搜索框

- 高 32px、圆角 8px、1px border；
- 左侧搜索图标 14×14px，距左 9px；
- 输入起点距左 30px；
- 右侧仅在有文字时显示清除按钮，命中框 28×28px；
- 输入后 150ms 防抖；
- 搜索项目路径、文件名、Class、函数、变量；
- `Esc` 第一次清空文字，第二次把焦点还给编辑器；
- 无结果时显示“当前范围内没有匹配项”，不能显示空白页。

### 18.6 路径与节点卡片

图路径参数直接沿用共享核心：

- 纵向路径线 x=`30px`；
- 一级卡片左边 x=`30 + 12 = 42px`；
- 深度每加一级，左边增加 `16px`；
- 每个节点布局槽高 `58px`；
- 卡片实际高 `46px`，上下各留 `6px`；
- 卡片右边距 `8px`；
- 卡片圆角 `8px`，边框 `1px`；
- 主标题距卡片上 `7px`；
- 副标题距主标题基线 `18px`；
- 图标为 14×14px，距卡片左 `8px`；
- 标题起点为图标右边再加 `7px`。

卡片类型标识：

| 节点 | 图标/缩写 | 主标题示例 | 副标题 |
|---|---|---|---|
| 项目 | `P` 或 project codicon | `TC-IDE` | 项目相对路径 |
| 文件夹 | folder codicon | `src/editor` | 包含文件数 |
| 文件 | file-code codicon | `main.rs` | 相对路径 |
| Class/Struct | symbol-class codicon | `GraphController` | `class` / `struct` |
| 函数 | symbol-method/function | `build_graph()` | `main.rs:42` |
| 变量 | symbol-variable | `project` | 定义类型与行号 |
| 调用 | references codicon | `render_graph()` | `调用 · main.rs:58` |
| 定义 | symbol-key codicon | `project` | `定义 · main.rs:7` |

只有确实存在子节点的卡片才显示折叠箭头：折叠为右向 chevron，展开为下向 chevron。叶子节点左边必须为空，绝不能显示 `V`。点击 chevron 只折叠；点击卡片其余区域选择并跳转。

### 18.7 卡片状态矩阵

| 状态 | 背景 | 边框 | 文字 | 位移 |
|---|---|---|---|---:|
| Normal | raised | border | text | 0px |
| Hover | raised 与 text 混合 10% | border 与 accent 混合 45% | bright | y=-1px |
| Pressed | raised 与 background 混合 35% | border 与 accent 混合 65% | bright | y=+1px |
| Selected | selection | accent | bright | 0px |
| Disabled | raised，整体 55% opacity | border 与 background 混合 50% | muted | 0px |
| Keyboard focus | 保持原状态 | 外加 1px bright focus ring | 保持 | 0px |

混色公式为每个 RGB 通道线性插值并四舍五入，不能用 HSL 猜测近似颜色。

### 18.8 空、加载、错误状态

- 加载：内容中央显示 16px spinner，下面 8px 显示“正在分析当前范围…”；超过 2 秒才显示取消按钮；
- 空项目：显示 project 图标、“打开一个项目后显示关系”，下方提供真实命令按钮；
- 无 Provider：显示“当前语言未提供符号/调用层级”，并提供“打开扩展”命令；
- 错误：使用 errorForeground 和 1px 错误边框，显示简短原因、“重试”和“打开输出”；
- 取消：保留最后一份成功图并在 footer 显示“已取消更新”，不闪成空白。

## 19. 动画施工图

### 19.1 唯一运动分类

只允许三类运动，不得为每个控件发明新曲线：

| 角色 | 刚度 | 阻尼 | 最大越界 | 用途 |
|---|---:|---:|---:|---|
| Overlay | 300 | 19 | 0.04 | 菜单、浮层、提示 |
| Drawer | 235 | 17 | 0.08 | 关系图内容、抽屉、大面板 |
| Follow | 620 | 34 | 0 | 跟随光标/选择项的轨道 |

弹簧每帧按半隐式积分：

```ts
velocity += (target - value) * stiffness * dt;
velocity *= Math.exp(-damping * dt);
value += velocity * dt;
```

- `dt` 最大取 `0.1s`；
- Overlay 到场前从 12px 偏移开始；
- Drawer 到场前从 24px 偏移开始；
- Follow 不做额外位置偏移，只平滑追踪目标；
- 不透明度为 `clamp(0.25 + 0.75 * progress, 0, 1)`；
- 距目标 `<0.0008` 且速度 `<0.0008` 时直接落位；
- 动画使用 `requestAnimationFrame`，不可用 `setInterval(16)`；
- 只更新 `transform`、`opacity` 或 Canvas，不循环改布局尺寸。

### 19.2 范围与关系指示线

- 使用 Follow 弹簧；
- 保存 `currentX/currentWidth/velocityX/velocityWidth`；
- 每帧追踪目标文本的 `offsetLeft/offsetWidth`；
- 指示线高度 2px；
- 切换时线条横向滑行并同步改变宽度；
- 不允许旧线 fade out、新线 fade in，因为这会看成瞬移；
- `pointerleave` 后必须清掉 hover 状态，不能让按钮永久发亮。

### 19.3 节点重新排序和折叠

- 每个节点以稳定 node ID 保存自己的 y 与 velocity；
- 新节点从最终 y+12px、opacity 0.25 进入；
- 旧节点向父节点方向收拢后淡出；
- 折叠时子节点不可立即 `display:none`，先在 160–260ms 内聚拢；
- 动画结束后再移出可访问树；
- 快速重复折叠时从当前数值继续，不重置到开头；
- 只渲染可见节点及上下各 3 行缓冲。

### 19.4 编辑反馈

VS Code API 不能移动原生编辑器中的单个字符，因此不得承诺复刻原生逐字上浮。插件版采用以下等价反馈：

- `onDidChangeTextDocument` 后对真实变更范围添加 Decoration；
- 总时长 260ms；
- 同一行 120ms 内相邻输入合并为一个 pulse；
- 最多同时保留 4 个 pulse；
- 插入使用 accent 低透明底色从 22% 降至 0%；
- 删除在删除前无法保留字形，因此只在原位置画 2px muted→透明短线；
- 粘贴多行算一个操作，不逐字符制造几百个 Decoration；
- 快速输入时若事件频率超过 20 次/秒，暂时只保留最新 pulse；
- 减少动效时不动画，只显示 80ms 静态变化提示。

### 19.5 语义关系出现顺序

右侧 Canvas 中一条关系路线按四阶段出现：

| 阶段 | 总进度区间 |
|---|---|
| 从源节点离开 | `0.00–0.38` |
| 沿主干生长 | `0.18–0.68` |
| 到达目标层级 | `0.48–0.88` |
| 分支展开 | `0.32–1.00` |

每段使用 `t² × (3 - 2t)` smoothstep。路径先主后支，禁止所有连线同一帧爆炸出现。

编辑器本体与 Webview 之间无法跨区域绘线。代码侧只画范围框、gutter 标记和 CodeLens；完整线条从 Webview 内的“当前符号”入口开始，不画到 VS Code DOM 外部。

## 20. 打包动画精确说明

### 20.1 画布与核心

- 动画使用单个 Canvas；
- 核心中心：`x=canvasWidth×0.5`，`y=canvasHeight×0.44`；
- 核心最大半径：`min(54px, canvasWidth×0.15)`；
- 共 7 圈手绘环；
- 环半径依次为核心半径的 `0.28, 0.40, 0.52, 0.64, 0.76, 0.88, 1.00`；
- 线宽在 `1.0–2.2px` 间按环交替；
- 每圈不是完美圆，使用 12 个控制点，半径扰动固定在 `±3%`，随机种子来自项目 ID，避免每帧闪烁；
- 黑洞顺时针，白洞逆时针；
- 黑洞暗色背景下使用 raised 圆盘托底、shadow 环线与 1px accent 外缘，确保黑线可见；
- 白洞使用 bright/selectedText 环线，不使用大面积纯白闪屏。

### 20.2 吸入对象

- 对象来自真实项目快照：文件、函数、Class、变量；
- 最多显示 36 个，多余对象按文件夹聚合；
- 文件用 14px file-code 线性图标；函数用 14px symbol-function；变量用 14px symbol-variable；Class 用 14px symbol-class；
- 不使用白色实心方块；
- 不绘制对象到黑洞的关系线；
- 对象在外圈移动慢，接近核心明显加速；
- 位移进度使用 `travelled^2.35`；
- 尾迹长度最大为原始距离的 10%；
- 横向摆动最大为原始距离的 3.2%，到达核心时降为 0；
- 尾迹短于 3px 不绘制；
- 所有对象在总进度 92% 前到达核心。

吸入时间：

```text
seconds = clamp(1.35 + sourceLines×0.006 + selectedTargets×0.12, 1.5, 5.5)
```

真实构建慢于吸入动画时，黑洞保持低速旋转并显示当前真实阶段；真实构建完成前禁止进入吐出阶段。

### 20.3 真实任务状态

严格状态机：

```text
idle → preparing → absorbing → building → packaging → emitting → success
                                     └──────────────→ failure
任意运行态 ───────────────────────────────────────→ cancelled
```

- `preparing`：检查目标和工具；
- `absorbing`：只在任务已真正启动后进行；
- `building`：显示真实 TaskExecution 状态和当前命令摘要；
- `packaging`：显示实际目标格式；
- `emitting`：只有退出码为 0 且产物存在才进入；
- `success`：显示产物名、大小、“打开目录”；
- `failure`：显示失败阶段、退出码、“打开 Output”；
- `cancelled`：立即停止粒子生成，已有对象在 120ms 内淡出。

### 20.4 白洞吐出

- 产物从核心向左右两列展开；
- 偶数索引向左，奇数索引向右；
- 多行产物在 `-90px..+90px` 间均匀分布；
- 横向目标距离默认 200px，窄视口改为视口宽度的 32%；
- 每个产物延迟为总阶段的 8%；
- 进度为 `1-(1-t)^2.6`；
- 产物卡片包含平台线性图标、格式、文件名和大小；
- 最终没有真实文件时不得吐出“安装包”。

## 21. 编辑器语义标识精确规则

### 21.1 角色与文本

不能把定义写成“调用/引用”。角色严格分为：

- 定义：声明变量、函数、Class 的位置；
- 调用：执行函数/方法的位置；
- 引用：读取、传递或写入符号但不调用的位置；
- 被调用：当前符号调用出去的目标；
- 调用者：调用当前符号的来源。

标签必须使用这些中文，不得混为“调用/引用”。数据来源优先为 Language Provider/LSP；无法判断时只显示“引用”，不能猜成调用。

### 21.2 Decoration

- 当前符号范围：1px accent 边框，圆角 3px，selection 20% 透明背景；
- 定义：左侧 gutter 使用 symbol-key，文字旁显示“定义”；
- 调用：使用 references 图标，文字旁显示“调用”；
- 引用：使用 link 图标，文字旁显示“引用”；
- 同时最多突出当前符号和 12 个最近关系，超过后只在关系图显示；
- 标签优先放在行尾 `after`，不能盖住正文；
- 行尾空间不足时改成 CodeLens，不叠在代码上；
- 编辑器失焦后保留光标符号焦点，但鼠标 hover 框在 300ms 后消失。

### 21.3 调试变量

- 仅 Debug Session 暂停时出现；
- 格式为 `name = value`；
- 值最长 80 个字符，超出显示省略号并在 Hover 中给完整值；
- 对象/数组显示一层摘要，不递归铺满编辑器；
- 变量标签使用 raised 背景、1px border、8px 圆角；
- 标签与变量之间只在同一编辑器行内画短引导线，不跨窗口；
- Step/Continue/Stop 后立即清除上一帧变量值；
- 无法从 DAP 映射到源码范围时只在 Variables/Debug View 显示。

### 21.4 注释

- 注释气泡不是本扩展第一阶段必须功能；
- 若实现，必须由语言语法/LSP 判断注释，不能把 C/C++ `#include` 当注释；
- 气泡不重复用箭头指向已经明显相邻的注释；
- 编辑注释时只修改注释正文范围，不能越过 `//`、`/* */`、`#` 等真实语法边界；
- 多行内容可折叠，折叠箭头只在内容超过两行时出现。

## 22. 无障碍、键盘与命中区域

- 所有按钮最小命中框 28×28px；主要按钮最小 32px 高；
- 每个图标按钮必须有中文 `aria-label` 与 tooltip；
- Tab 顺序严格按视觉从上到下、从左到右；
- Enter/Space 激活按钮，Escape 关闭浮层或清空当前临时状态；
- 上下方向键移动图节点，左右方向键折叠/展开，Enter 跳转；
- 焦点不能被 Webview 困住；最后一个控件后允许 Tab 回到 VS Code；
- 文字与背景对比至少 4.5:1，装饰线至少 3:1；
- 高对比度主题使用 VS Code HC tokens，不强制 TC-IDE 普通色板；
- `prefers-reduced-motion: reduce` 时所有弹簧改为 0ms 或不超过 80ms 淡入淡出；
- 屏幕阅读器状态通过 `aria-live="polite"` 报告加载完成、失败和节点数量，动画帧不逐帧播报。

## 23. 截图与几何回归系统

### 23.1 必须提供的固定场景

建立独立的 Webview harness，和正式扩展导入同一个渲染模块，不复制第二份 HTML。场景至少包括：

1. `graph-dark-compact`：宽 280、高 820、暗色、当前文件范围；
2. `graph-dark-expanded`：宽 460、高 900、暗色、整个项目范围；
3. `graph-light-expanded`：宽 460、高 900、亮色；
4. `graph-empty`：无项目；
5. `graph-loading`：加载超过 2 秒；
6. `graph-error`：Provider 报错；
7. `graph-folded`：文件夹折叠；
8. `graph-search`：搜索命中 Class、函数和路径；
9. `package-absorbing`：吸入进度 0.55；
10. `package-building`：真实构建等待态；
11. `package-emitting`：吐出进度 0.62；
12. `package-failure`：退出码非零；
13. `reduced-motion`：减少动效；
14. `high-contrast`：高对比度。

测试必须可以冻结动画时间，例如 `?testTime=0.55`。截图时禁止依赖真实系统时间和随机数。

### 23.2 几何断言

截图之外还要读取 DOM/Canvas 布局并断言：

- 根区域无水平溢出；
- 标题行高 32±1px；
- 控件高 32±1px；
- 卡片槽高 58±1px；
- 卡片深度差 16±1px；
- 一级卡片距左 42±1px；
- 路径线距左 30±1px；
- 指示线高 2±0.5px；
- 所有交互命中框至少 28×28px；
- footer 高 34±1px；
- 任何文字不与图标或另一段文字重叠；
- 叶子节点没有 chevron；
- 浮层层级高于内容，但不能盖住 VS Code 原生 Panel。

### 23.3 截图差异规则

- 基准由项目所有者或 Codex 首次批准；
- Chromium 版本在 CI 固定；
- 字体抗锯齿区域允许建立 mask；
- 非文字区域像素差异比例不得超过 0.5%；
- 全图差异比例不得超过 1.5%；
- 单个连续差异块若大于 8×8px，即使总比例很低也失败；
- 失败时输出 expected、actual、diff 三张图；
- 禁止通过提高容差掩盖错位；调整容差必须单独提交并解释原因。

### 23.4 动画验收

自动采样每个动画的 0%、25%、50%、75%、100%：

- 位置必须单调向目标收敛，允许弹簧规定范围内越界；
- Follow 不允许越界；
- 切换中途再次操作不能跳回初始位置；
- 60Hz 环境平均帧间隔目标 ≤18ms；
- 99% 帧间隔目标 ≤34ms；
- 面板不可见时不得继续 `requestAnimationFrame` 循环；
- 测试同时记录 DOM 节点数、Canvas draw 次数与 Extension Host 请求数。

## 24. 视觉验收清单

DeepSeek 提交给 Codex/项目所有者前逐项回答“是/否/不适用”，不能留空：

- [ ] 深色背景是蓝灰层级，不是纯黑大块堆叠；
- [ ] 亮色主题不是简单反色，语法颜色使用独立亮色值；
- [ ] 界面只有一个主要 accent，没有彩虹式按钮；
- [ ] 关系切换使用移动指示线，不是三个大按钮瞬切；
- [ ] hover 从上/下/左/右离开后都会清除；
- [ ] 只有可折叠节点显示 chevron，叶子节点没有 `V`；
- [ ] 项目、文件夹、文件、Class、函数、变量层级能从缩进看懂；
- [ ] 定义、调用、引用文字准确，没有统一写成“调用/引用”；
- [ ] 卡片没有遮挡路径、标题和行号；
- [ ] 搜索、空状态、加载、错误、取消都有真实反馈；
- [ ] 快速滚动不触发全量索引；
- [ ] 动画只使用 transform/opacity/Canvas，不靠频繁布局重排；
- [ ] 打包动画与真实 Task 状态绑定；
- [ ] 构建失败不会吐出假安装包；
- [ ] 成功后能打开真实输出目录；
- [ ] 终端使用 VS Code Terminal，不是 Webview 文本；
- [ ] 深色、亮色、高对比度、减少动效都通过；
- [ ] 截图基准由有视觉能力的人批准，而不是实现者自批；
- [ ] README 明确说明 VS Code API 限制造成的适配差异。

## 25. 实际完成记录

尚未开始。第一步应创建 `vscode-extension/` 骨架并完成阶段 A，不得先制作静态 Webview 演示来冒充完整扩展。视觉实现必须从第 17 节 Token 和第 18 节关系图施工图开始，第一批截图只能标记为“待视觉批准”。

### 2026-10-18 / 阶段 A（可运行骨架）

**完成**：

- `vscode-extension/` 独立 npm 工程：TypeScript 入口 + esbuild 双 bundle（Extension Host 用 CJS 并 external `vscode`；Webview 用浏览器 IIFE，已实测**不含任何 `vscode` 引用**）。
- Manifest：Activity Bar 容器 `tcIde` + WebviewView `tcIde.graphView` + 5 个命令 + 10 项配置 + 2 套主题 + `viewsWelcome`。
- TC-IDE Dark / Light 两套完整主题（颜色逐项取自 `src/theme_core.rs`），中英双语 l10n bundle。
- 关系图 Webview 按第 17 节 Token 与第 18 节施工图实现：标题行 32、关系轨道 32（指示线为**单个移动元素**，不是三个按钮瞬切）、范围浮层（选中用 2px accent 短线，不用字母）、搜索框（150ms 防抖、Esc 两级）、面包屑 24、视口、footer 34；卡片槽 58、一级卡片距左 **42**、每级右移 16、卡片高 46、**叶子节点无 chevron**；加载/空/无 Provider/错误四态互斥。
- `.tcIDE` 文档：创建命令、解析（未知字段告警、版本过高报错）、序列化。
- 语义数据全部走真实 VS Code API：`executeDocumentSymbolProvider`、`prepareCallHierarchy`、`provideIncomingCalls/OutgoingCalls`；无 Provider 时返回 `degraded` 并在视图说明，**不猜测关系**。
- 性能：provider 端 300ms 防抖 + 可取消 + 序号丢弃过期回答；Webview 只渲染可见行 + 上下各 3 行缓冲；动画循环在静止后停止。
- **截图/几何 harness**：`src/graph/harness.ts` 与扩展复用同一份 `view.ts` markup/styles（无第二份 HTML）；10 个场景；`?testTime=` 可冻结时间。
- **真实浏览器几何断言**：`scripts/geometry-check.mjs` 用 Chromium 读 DOM 断言 **187 条**，全部通过。
- VSIX 打包检查 `scripts/verify-package.mjs`：清单指向的文件必须存在、主题必须可解析、bundle 必须包含所有命令与视图、无密钥特征。
- CI：`.github/workflows/vscode-extension.yml`（typecheck / lint / test / build / geometry / package / 上传 vsix 与截图），**不创建 Release**。

**修改文件**：`vscode-extension/` 全部为新增；另新增 `.github/workflows/vscode-extension.yml`。`src/` 下的原生程序**未改动**。

**验证命令与结果**（均在 `vscode-extension/` 下执行）：

| 命令 | 结果 |
|---|---|
| `npm run typecheck` | 通过 |
| `npm run lint` | 通过（0 warning） |
| `npm test` | **30 通过 / 0 失败** |
| `npm run build` | 两个 bundle 成功 |
| `node scripts/geometry-check.mjs` | **187/187 断言通过** |
| `node scripts/verify-package.mjs --build` | 通过，vsix **27 412 字节 / 11 个文件** |

**截图**：`vscode-extension/harness-shots/` 共 10 张，**两次运行逐字节一致**（可作回归基线）。

> ⚠️ **这些截图尚未经视觉批准。** 实现者所用模型**不具备图像输入能力**，无法查看 PNG，
> 因此**不能**由实现者确认「视觉一致」。按第 15.2 节，首批基准必须由项目所有者或有视觉能力的
> Codex 查看后批准。在那之前状态是**「等待视觉验收」**，不是「视觉一致」。
>
> 已用数值代替肉眼的部分：截图主色像素恰好等于主题 Token（暗色 `#0D1219` 304 205 px、
> `#1D2937` 95 335 px；亮色 `#F5F5F7` 337 253 px、`#F2F2F7` 65 865 px），证明 Token 真的生效；
> 文字对比度由 `theme.test.ts` 断言 ≥4.5:1。

**过程中发现并修掉的真实缺陷**（均由测试或工具抓出，非猜测）：

1. 消息 `version: "1"`（字符串）曾被 `Number()` 强制转换通过 → 改为严格要求 `typeof === 'number'`。
2. CSP 中 `style-src 'nonce-…' 'unsafe-inline'` **互相抵消**，内联 `element.style` 全被拦截 → 样式改用 `'unsafe-inline'`，脚本保持 nonce-only。
3. 卡片因根元素 12px 内边距被定位到 54px 而非 42px → 绘制时减去该内边距。
4. 加载态与错误态各自会与「空」面板**同时显示** → 状态面板改为互斥。
5. `graph-error` 与 `graph-empty` 截图曾**逐字节相同**：harness 先判 `snapshot === null` 走错分支，且浏览器 `--screenshot` 在消息到达前就截图 → 修正分支顺序，改用 Puppeteer 等 `__TC_HARNESS_READY__` 后再截。

**未完成（不要当成已做）**：

- 编辑器内 Decoration / CodeLens / InlayHints（阶段 C）；
- 调试变量内联显示（阶段 C）；
- TaskProvider 与 Build/Run/Package（阶段 D）；
- 黑洞/白洞打包动画（阶段 E）——**一行都还没写**；
- 多目标打包选择；
- 大项目性能实测（只有设计上的限流，**没有基准数字**）；
- 视觉基准批准。

**下一步**：把 `harness-shots/` 交给项目所有者或 Codex 做视觉批准；并行可开始阶段 B 的
Call Hierarchy 双向联动与阶段 C 的 Decoration。
