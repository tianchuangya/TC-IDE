# TC-IDE Companion

把 TC-IDE 的语义关系图带进 VS Code。这是一个**独立扩展**，不修改也不嵌入原生 TC-IDE。

## 功能

- **调用层级视图**（右侧 Webview）：项目 → 文件夹 → 文件 → 类 → 函数/变量，缩进表示层级。
- **关系方向**：跟随、调用者、被调用。
- **范围**：自动、整个项目、当前文件夹、当前文件、当前符号。
- **搜索**：在面板内按路径、文件名、类、函数、变量过滤。
- **跳转**：点击节点打开真实文件并定位行列。
- **主题**：TC-IDE Dark / TC-IDE Light（颜色与原生 `theme_core` 一致）。
- **项目文档** `.tcIDE`：记录默认范围、关系、任务名与打包目标。

## 快捷入口

| 命令 | 说明 |
|---|---|
| `TC-IDE: 打开语义关系图` | 显示视图并刷新 |
| `TC-IDE: 刷新关系图` | 只重新请求当前范围 |
| `TC-IDE: 切换关系范围` | 打开范围选择 |
| `TC-IDE: 切换关系方向` | 打开方向选择 |
| `TC-IDE: 创建 .tcIDE 项目文档` | 在工作区根目录生成 |

## 本阶段**尚未**实现（明确列出，不假装已完成）

- 编辑器内的定义/调用/引用 Decoration 与 CodeLens；
- 调试变量内联显示；
- Build/Run/Package 任务提供器与黑洞/白洞打包动画；
- 扩展端以外的语言能力（它使用 VS Code 已有的语言服务）。

## VS Code API 造成的适配差异

- 扩展**不能**接管顶部菜单栏、标签栏或编辑器布局，也不会注入 DOM。
- 编辑器内不能绘制跨区域连线，因此完整关系只画在视图里；代码侧只用 VS Code 原生提示表达。
- 视图可以被用户移动到侧边栏任意位置，扩展不会强制移动它。
- 没有语言服务的语言会显示「当前语言未提供符号或调用层级」，**不会**猜测关系。

## 隐私

- 不联网、不收集数据、不写日志到磁盘。
- 不在 `.tcIDE` 中保存密钥；扩展本身不读取任何密钥。

## 设置

见 `package.json` 的 `contributes.configuration`：`tcIde.graph.*`、`tcIde.editor.*`、`tcIde.package.animate`、`tcIde.reducedMotion`。

## 故障排查

- **视图为空**：先打开一个文件。没有语言的符号提供者时视图会说明原因。
- **关系图没有调用者/被调用**：该语言的扩展需要提供 Call Hierarchy。
- **安装**：`code --install-extension tc-ide-companion.vsix`

## 开发

```bash
npm install
npm run build        # 打包 Extension Host 与 Webview 两个 bundle
npm run typecheck
npm run lint
npm test
node scripts/capture-harness.mjs --serve   # 打开几何/截图 harness
node scripts/geometry-check.mjs            # 真实浏览器里的几何断言
```
