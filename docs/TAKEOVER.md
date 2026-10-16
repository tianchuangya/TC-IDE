# TC-IDE 交接说明（给接手的人 / 下一个 AI）

> 本文写于 2026-10。**先读这一页，再读 `docs/HANDOFF.md`。**
> `docs/HANDOFF.md` 是需求队列与历史；本文是「现在是什么状态、怎么验证、下一步做什么」。

---

## 0. 三十秒版本

- 仓库：`/home/tianchuangya/桌面/TC-IDE`，分支 `main`，与 `origin/main` 同步，工作区干净。
- 测试：**317 + 3 + 4** 全绿；三目标 clippy 零警告；CI **passing**。
- 队列：**44 已完成 / 2 部分完成 / 0 未开始**。
- 两个部分完成项**本机做不到**（需要 Windows 实机 / 一台 Mac），不要试图在 Linux 上「补完」它们。
- **刚完成的一件事**：项目插件清单、可执行命令与扩展面板已接通（见 §4.8）。
- **方向已变**：用户明确说不再为 Windows 平台对齐投入时间，**专注新功能**。

---

## 1. 必须先知道的三个坑

### 1.1 `cargo` 有两个版本，用错会出假结果

```bash
which cargo                      # /home/tianchuangya/.local/bin/cargo  → 1.75.0
~/.cargo/bin/rustup run stable cargo --version   # 1.99.0  ← 必须用这个
```

默认 `cargo` 是 1.75，**格式化结果与 CI 不同**（曾因此让 CI 真的红过一次）。所有命令都要走 `rustup run stable`。

### 1.2 Windows 验收本机做不了，但状态层能跑

本机是 Linux。**不能**在 Windows 上跑真实窗口。但可以：

```bash
export PATH="/tmp/mingw/root/usr/bin:$HOME/.cargo/bin:$PATH"
bash tools/windows_selfcheck_wine.sh
```

它会在 Linux 上**编译真实的 Windows 后端**（含 `windows` crate 与 Direct2D）并在 Wine 里跑 **44 项状态层自检**。观感（按下/禁用/焦点环、高 DPI、IME、多显示器）**仍只能在实机由人确认**，清单见 `docs/REAL-MACHINE-CHECKLIST.md`。

> 依赖 `/tmp/mingw/root` 与 `/tmp/tc-ide-winbuild`，**重启后会丢**。重建配方写在 `docs/KNOWN-ISSUES.md`。核心是 `apt-get download` + `dpkg-deb -x`（**不需要 root**）+ Wine。

### 1.3 macOS 连链接都做不到

缺 macOS SDK（没有合法获取途径），沙箱里的 macOS 二进制还需签名。**只能**：

```bash
~/.cargo/bin/rustup run stable cargo check --all-targets --target x86_64-apple-darwin
```

CI 里有 macOS job，会**构建并运行** `--self-check` 与单元测试——这是 macOS 代码唯一的真实执行环境。**我这两轮的两个逻辑错误就是它抓出来的**，不要因为它慢就忽略。

---

## 2. 提交前必须跑的验证（顺序固定）

```bash
cd /home/tianchuangya/桌面/TC-IDE
R=~/.cargo/bin/rustup

$R run stable cargo fmt --check
$R run stable cargo test --all-targets
$R run stable cargo clippy --all-targets -- -D warnings
$R run stable cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings
$R run stable cargo clippy --all-targets --target x86_64-apple-darwin -- -D warnings
python3 tools/windows_editor_sim.py          # 模块边界守卫，见 §3.2
```

**测试通过 ≠ 可以推送。** 上面六条全过才算。第 21 轮我在 clippy 失败时推送过，教训写在 `docs/PROGRESS-TRANSFER.md`。

推送后等 CI：

```bash
curl -s -L "https://github.com/tianchuangya/TC-IDE/actions/workflows/build.yml/badge.svg?branch=main" | grep -o "<title>[^<]*</title>"
```

GitHub API 对本会话有**未认证限流（403）**，所以只能用 badge/HTML 页面判断，不要试 API。

---

## 3. 架构约束（不要让它们变松）

### 3.1 共享核心不得依赖平台

18 个跨平台核心列在 `tests/platform_boundary.rs` 的 `SHARED_CORES`。它们**必须**能在 Linux 上编译并单测，禁止出现 Win32/X11/Cocoa 引用。

**新增共享核心时，要把它加进这个列表**（`search_core.rs` 是最近一个例子）。

### 3.2 `editor_core` 是叶子模块

`editor_core` **不得**依赖 `syntax_core`（依赖方向是 `syntax_core` → `editor_core`）。

`tools/windows_editor_sim.py` 只链接 `text_buffer` + `editor_core` 两个模块，所以**任何向上的依赖都会让它编译失败**。我做括号配对时踩过：`editor_core` 调 `syntax_core::mask_line` 直接挂了，正确做法是**由调用方把掩码传进来**。**不要为了让模拟器通过而放宽它。**

### 3.3 `windows_app/*.rs` 每文件 ≤1000 行，根 ≤1500 行

由 `tests/module_layout.rs` 强制。超了要**拆代码**，不是放宽限制（历史做法：把 `focus_settings_row_at` 抽出去）。

### 3.4 渲染帧门槛

CI 会渲染所有场景，要求每帧 >20 000 字节，且（除最后两个场景外）与编辑器帧不同。

**加了可见改动就加场景**，并把它加进 `.github/workflows/build.yml` 的帧列表。现有场景：

CI 里逐个渲染并检查的场景（权威列表在 `.github/workflows/build.yml`）：

```
(default)  packaging  packaging-emit  edit  debug  view  file-menu
ai-panel  diagnostics  brackets  folding  find  project-find
graph-callees  relations  background-crop  settings-hover  settings-controls
syntax-colors
scrolling
plugins
```

另有 `background` 场景可在源码中使用，但未进入 CI 帧门槛。

渲染命令（不需要窗口映射）：

```bash
DISPLAY=:0 cargo run -- --render-frame /tmp/x.png --render-scene <名字>
```

**验证渲染改动的方法**：先渲染一帧，再临时改代码/场景渲染第二帧，然后 `ImageChops.difference` 出 bbox 和变化像素数。**不要靠肉眼看颜色值**——抗锯齿会混合，要按区域统计或做差分。

---

## 4. 已完成：Linux 控件观感

用户批准的三项修改已经实现并通过本地帧验证。

### 4.1 已确认的问题（实测，非推测）

Linux 的 `settings_control_colors()`（`src/linux_app.rs:1527`）**自己手写了一套状态→颜色映射**，而 **Windows 和 macOS 用的是共享的 `theme_core::Palette::control()`**。

```bash
grep -c "palette.control(" src/linux_app.rs        # 0
grep -c "palette.control(" src/windows_renderer.rs # 3
grep -c "palette.control(" src/macos_app.rs        # 1
```

实测像素（场景 `settings-hover` 与 `settings-controls`，差分 11 460 像素）：

| 状态 | Linux 现在的填充 | Linux 现在的边框 |
|---|---|---|
| Normal | `#2c2c2e` (`colors.panel_2`) | line / accent |
| Hovered | `#38383a` (`colors.line`) | accent |
| **Pressed** | **`#000000` (`colors.bg`)** | accent |
| Disabled | `#2c2c2e` | line |

三个问题：

1. **按下跳到纯黑** `#000000`，比常态 `#2c2c2e` 还暗，和"悬停变亮"方向相反——像渲染错误。
2. **常态→悬停只差 3.6% 亮度**，深色主题上几乎看不出。
3. **禁用态填充与常态完全相同**，只有文字变灰。另外**焦点环与悬停边框都是 accent**，两者同时出现时分不清哪个是焦点。

### 4.2 用户批准的三项修改

1. **按下态不要纯黑**：悬停 `#38383a`（亮一档）→ 按下回到接近常态，但**保留 accent 边框 + 下沉 1px**。（下沉已实现，`draw_settings_control` 里有 `y + s(1)`。）
2. **禁用态压低整体不透明度**，让"不可用"一眼可见。
3. **焦点环与悬停必须能区分**——这是用户认为最该修的一处。

### 4.3 关键发现：**共享模型里已经写好了**

`theme_core::Palette::control(state, emphasised) -> ControlVisual`（`src/theme_core.rs:228`）**已经实现了上面三项**：

- `Hovered`：非强调控件填充 `mix(base, text, 0.10)`，边框 `mix(border, accent, 0.45)`，**lift +1.0**
- `Pressed`：填充 `mix(base, background, 0.35)`（不是纯黑），**lift −1.0**（下沉）
- `Disabled`：**`opacity: 0.55`**，文字 muted

`ControlVisual` 有 `fill / border / text / lift / opacity` 五个字段。

**所以这大概率不是"写新逻辑"，而是"让 Linux 改用它已有的共享实现"**——和 27–36 轮反复出现的同一类问题：*共享代码写好了，某个后端没接上*。

### 4.4 实现与证据

- Linux 删除私有的状态配色分支，启动时把 `Palette::control()` 的八种状态/强调组合转换为 X11 pixel 并缓存；绘制直接采用共享 `fill/border/text/lift`。
- `ControlVisual::composited_on()` 把禁用态的 `opacity: 0.55` 真实合成到面板底色，适配 X11 不带 alpha 的矩形原语。
- 按下边框比悬停更靠近 accent，焦点环改用共享 `focus_ring()`（bright），因此键盘焦点不再与鼠标悬停同色。
- 与交接前留在 `/tmp` 的帧比较：`settings-hover` 改变 12,100 像素，`settings-controls` 改变 11,633 像素；按下填充从 `#000000` 变为 `#1d1d1e`，按下边框为 `#1a69ba`，悬停填充为 `#3d3d3f`。
- 共享测试断言按下/悬停边框不同、焦点/悬停不同、禁用透明度合成后成为不透明且仍区别于常态。

### 4.5 代码关系与工作区恢复

- 代码中的“定义 / 调用 / 引用”改为不透明的小型指向气泡，不再用透明文字直接压住后续代码。
- Rust 字段等 `project: Project` 形态会标成“定义”，函数调用和普通引用分开显示。
- 关系扫描经过 `syntax_core::mask_line()`，字符串和注释里的同名词不再产生箭头。
- 新增 `relations` 渲染场景，并纳入 Linux CI 帧门槛。
- 退出窗口时保存活动文档、光标、项目树展开状态、调用层级显示/宽度、终端显示/高度；尺寸以窗口千分比保存，换分辨率后仍可用。

### 4.6 语法高亮颜色自定义

- 设置页顶部的“语法颜色”打开独立面板，可分别调整关键字、类型/Class、函数、常量、变量、参数、属性、宏/注解、模块/命名空间、预处理、字符串/数字和注释。
- 同时支持 18 色调色盘与 `#RRGGBB` / `RRGGBB` 输入，有效值立即刷新代码预览。
- 深色、浅色主题分开保存自定义值，单项可恢复主题默认色；配置保存在 `syntax-colors.conf`。
- 新增 `syntax-colors` 渲染场景并纳入 Linux CI 帧门槛。

### 4.7 代码滚动与真实 Build/Run

- Linux 代码区现在有持久的纵向视口偏移，支持滚轮、点击滚动轨道和拖动滚动条；光标用方向键越过视口时会自动跟随。
- 右侧终端列表的 `Build` / `Run` 是真实操作：先保存当前文件，再在项目根目录执行语言对应命令。
- C/C++/Java 的 Run 命令包含“编译 `&&` 运行”，首次运行不再依赖事先存在的二进制；Rust/Go/Python/Node 分别使用 Cargo/Go/Python/npm。
- `Ctrl+F5` 可直接执行 Run；F5 仍保留为调试。
- CI 视觉场景 `scrolling` 固定验证长文件视口和滚动条。

### 4.8 项目插件基础

- 共享 `extension_core::parse_plugin_manifest()` 支持 `[plugin]` 和重复的 `[command]` 段。
- 项目把 `.plugin` 或 `.conf` 清单放在 `.tc-ide/plugins/`；打开项目时会自动扫描。
- 扩展 → 项目插件中显示版本、描述和命令；用户点击命令后才会在 Run 终端执行，同时写入 Output 汇总。
- 清单是用户项目的明确配置，TC-IDE 不会自动执行未点击的外部命令。

---

## 5. 我这几轮做了什么（背景，避免重复劳动）

### 5.1 审计方法（最有用的一条）

从第 27 轮起我用同一个方法找到 **7 个真实缺陷**：

> **列出共享核心的公开 API，逐个数每个后端实际用了几个。**

```bash
# 例：谁用了 palette 的控件状态机
for f in src/linux_app.rs src/windows_renderer.rs src/macos_app.rs; do
  echo "$f: $(grep -c 'palette\.control(' $f)"
done
```

发现的缺口类型：

| 缺陷 | 性质 |
|---|---|
| 终端选择只有 Windows 有 | 功能缺失 |
| 关系图每帧重扫所有文件 **142ms/帧** | 性能（改后 ≈8ms/帧） |
| Linux 少一个图范围、缺「自动」态 | 功能不对等 |
| macOS 无编辑动画 | 功能不对等 |
| Linux 无 AI 面板 | 整块缺失 |
| Linux 无撤销/重做 | 基础功能缺失 |
| Linux 无语言服务器 | 核心功能缺失（后补齐 6 项） |

**根因规律**：队列里 `[x]` 记的是「当初打算做到**哪一端**」，不是「三端现在都做到了」。看到 `[x]` 不要相信，去数。

> **注意**：`grep hover` 之类的关键词搜索**会骗你**。我先用 `grep -c hover` 得 0，误以为 Linux 没实现控件状态；实际上字段叫 `settings_pressed`，状态机在 `settings_control_state()`。**要读函数，不要只 grep 关键词。**

### 5.2 用户转向后交付的新功能（27 项新测试）

用户在第 37 轮说：**不要再为 Windows 平台对齐花时间，专注新功能**。

| 功能 | 测试 | 快捷键 |
|---|---|---|
| 括号配对（跨行、只认同种、忽略注释与字符串） | 6 | — |
| 代码折叠（单行不折叠、嵌套深度、连同嵌套一起隐藏） | 5 | `Ctrl+Shift+[` / `]` |
| 跳转到行（非法输入拒绝而非夹取、列按字符） | 4 | `Ctrl+G` |
| 文件内查找替换（不重叠、回绕、全字、可撤销） | 12 | `Ctrl+F` / `Ctrl+H` / `F3` |
| 项目内跨文件搜索 | 5 | `Ctrl+Shift+F` |

新增共享核心 `search_core`（已纳入边界守护，现共 **18 个**）。

### 5.3 已被工具抓出的我自己的错误（值得你警惕）

- **macOS 自检抓出两次**：编辑动画我按「光标所在行」取高亮，但回车会把编辑记在**上一行**——最该有反馈的操作反而没有。修法是共享模型加 `newest()`。
- **Windows 模拟器抓出一次**：`editor_core` 反向依赖 `syntax_core`，违反叶子模块约束。
- **断言写错了三次**（删除坐标、`to_lines` 丢弃末尾空行的既有行为、redo 次数）。三次我都是**先核实真实行为再改断言，而不是改代码**。

---

## 6. 硬约束（不要违反）

- **不创建标签、不发 Release、不触发 `release.yml`**（它只能手动触发，仓库里有 2 个**历史遗留**标签，不要动）。
- **不使用 WebView / HTML / CSS 运行时**。
- **任何密钥不得出现在仓库、配置、日志、命令行里**。AI Key 只走环境变量；安装扩展的目录由用户提供（`<项目>/.tc-ide-catalog.conf`），**仓库不内置编造的 URL 与校验和**。
- **不执行破坏性 git 命令**。`git checkout -- <file>` 曾把我未提交的工作（文件菜单 + 展开控件）整个冲掉，只能重做。
- **保留用户自己的改动**。

---

## 7. 剩下什么没做

### 7.1 两个部分完成项（本机无解）

1. **控件观感 + 高 DPI/IME/多显示器** —— 需 Windows 实机人工确认。清单位于 `docs/REAL-MACHINE-CHECKLIST.md`。
2. **macOS 的 Metal 渲染与直接 CoreText 排版** —— 需一台 Mac。我只能做 `cargo check`。

### 7.2 明确没有实现、且**不要假装实现了**的

- **正则搜索**：`search_core::Query` 里有 `regex` 字段，但匹配**仍是字面量**。代码注释和文档都写明了这一点——静默把正则当字面量会找错东西。真做需要独立工作量。
- **Tree-sitter 级增量解析**：语义关系是「语言感知扫描 + 返回类型推导」，**不是真正的语法树**。列在 `docs/HANDOFF.md` 第 6 节。
- **签名安装包**：流程与命令已实现，**需要用户自己的证书**。
- **项目内跨文件替换**（搜索已有，替换只在当前文件）。

### 7.3 候选新功能（按「让使用者少做一步手工」排序）

缩进参考线、多光标、项目内替换、查找正则。

---

## 8. 文档地图

| 文件 | 内容 |
|---|---|
| `docs/TAKEOVER.md` | **本文。先读它。** |
| `docs/HANDOFF.md` | 需求队列（§5）、方向变更（§11）、新功能进度（§11.1）、当前交接结论（§10） |
| `docs/KNOWN-ISSUES.md` | Windows/Wine 配方、macOS SDK 限制等已知问题 |
| `docs/REAL-MACHINE-CHECKLIST.md` | Windows 实机验收清单（给人执行） |
| `docs/PROGRESS-TRANSFER.md` | 历史踩坑记录 |
| `CHANGELOG.md` | 变更历史 |
| `tools/windows_selfcheck_wine.sh` | 在 Linux 上编译并自检 Windows 后端 |
| `tools/windows_editor_sim.py` | 模块边界 + 编辑序列守卫 |

---

## 9. 接手后的第一步（建议）

```bash
cd /home/tianchuangya/桌面/TC-IDE
git log --oneline -5
~/.cargo/bin/rustup run stable cargo test --all-targets   # 应为 313 + 3 + 4
# 然后读 §4，从 Linux 控件观感开始
```

有任何不确定的地方，**先在代码里核实，再写**。这份文档里的每条结论都是我实际跑出来或读出来的；但它由 AI 撰写，**难免有错**——`docs/HANDOFF.md` 里就曾有多处过期结论（我在第 36 轮逐项核对后重写了第 10 节）。**怀疑本文，去查代码。**
