# TC-IDE 架构草案

## 原则

TC-IDE 不是网页套壳。界面状态、布局、动画和绘制都由 Rust 控制；CSS 只属于早期概念预览，不进入产品运行时。

## 分层

1. `tc_workspace`：项目、目录、文件、符号索引。
2. `tc_semantic_graph`：项目 → 文件 → Class → 函数，以及调用、声明、弱定义与 AI 推断边。
3. `tc_editor`：文本缓冲、选择区、光标和字形级插入/删除动画。
4. `tc_ui`：原生窗口、GPU 场景、输入与统一动画时钟。
5. `tc_packager`：Windows、Debian、macOS、AppImage、RPM 构建任务和黑洞/白洞状态动画。

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

