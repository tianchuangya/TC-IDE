# TC-IDE 交接说明（给 Codex）

更新时间：2026-10-17
仓库：`https://github.com/tianchuangya/TC-IDE`
工作目录：`/home/tianchuangya/桌面/TC-IDE`
当前 HEAD：`3cc77b1`，与 `origin/main` 同步，工作区干净。

> 这份文档由接手的前一个 AI（DeepSeek）写给下一个（Codex）。**里面的每个数字和结论都在写之前跑过或读过代码**，
> 但它仍可能有错——怀疑时请自己核实，不要直接相信叙述。

---

## 0. 三十秒版本

- **测试**：`352 + 3 + 4` 全部通过。
- **三目标 clippy**（Linux / `x86_64-pc-windows-msvc` / `x86_64-apple-darwin`）零警告。
- **CI**：`build - passing`。
- **帧门槛**：25 个渲染场景，全部 >20 000 字节且互不相同。
- **队列**：`docs/HANDOFF.md` 第 5 节为 44 已完成 / 2 部分 / 0 未开始。那 2 项**本机做不到**（需要 Windows 实机 / 一台 Mac），不要试图在 Linux 上「补完」。

---

## 1. 三个必须先知道的坑

### 1.1 `cargo` 有两个版本，用错会得出假结果

```bash
which cargo                                        # /home/tianchuangya/.local/bin/cargo → 1.75.0
~/.cargo/bin/rustup run stable cargo --version     # 1.99.0  ← 必须用这个
```

默认 `cargo` 是 **1.75**，格式化结果与 CI 的 stable **不同**，历史上因此让 CI 真的红过一次。
**所有命令都要走 `rustup run stable cargo`。**

### 1.2 本机是 Linux，Windows 只能跑状态层

可以（需要先准备 mingw 与 Wine，配方见 `docs/KNOWN-ISSUES.md`）：

```bash
export PATH="/tmp/mingw/root/usr/bin:$HOME/.cargo/bin:$PATH"
bash tools/windows_selfcheck_wine.sh
```

它会在 Linux 上**编译真实的 Windows 后端**（含 `windows` crate 与 Direct2D）并在 Wine 里跑 **44 项状态层自检**。

**观感**（按下/禁用/焦点环、高 DPI、IME、多显示器）**只能在 Windows 实机由人确认**，
清单位于 `docs/REAL-MACHINE-CHECKLIST.md`。

> 依赖 `/tmp/mingw/root` 与 `/tmp/tc-winbuild`，**重启后会丢**。核心是 `apt-get download` + `dpkg-deb -x`
> （**不需要 root**）。本会话试过 `sudo`，不可用且不需要。

### 1.3 macOS 连链接都做不到

缺 macOS SDK（没有合法来源），沙箱里的 macOS 二进制还需签名。本机**只能**：

```bash
~/.cargo/bin/rustup run stable cargo check --all-targets --target x86_64-apple-darwin
```

CI 的 `macos` job 是**唯一的真实执行环境**。别因为它慢就忽略——它在本会话抓出过两次真实逻辑错误。

---

## 2. 提交前必须跑的验证

```bash
cd /home/tianchuangya/桌面/TC-IDE
R=~/.cargo/bin/rustup

$R run stable cargo fmt --check
$R run stable cargo test --all-targets
$R run stable cargo clippy --all-targets -- -D warnings
$R run stable cargo clippy --all-targets --target x86_64-pc-windows-msvc -- -D warnings
$R run stable cargo clippy --all-targets --target x86_64-apple-darwin -- -D warnings
python3 tools/windows_editor_sim.py
```

**测试通过 ≠ 可以推送。** 六条全过才算。我在本会话有过一次 clippy 未过就推送，导致 CI 红。

渲染帧门槛（**可见改动必须验证**）：

```bash
DISPLAY=:0 cargo run -- --render-frame /tmp/x.png --render-scene <场景名>
```

判定方法：先渲染一帧，改代码后再渲染一帧，用 `ImageChops.difference` 求 bbox 与变化像素数。
**不要靠肉眼比对颜色**——抗锯齿会混合，要按区域统计或做逐像素差分。

CI 的帧门槛规则（读 `.github/workflows/build.yml`）：每帧 >20 000 字节，且除编辑器帧外都要与它不同。
**加可见改动就要加场景并加进 `build.yml` 的文件列表**，否则门槛覆盖不到。

---

## 3. 架构约束（不要让它们变松）

### 3.1 共享核心不得依赖平台

**19 个**跨平台核心列在 `tests/platform_boundary.rs` 的 `SHARED_CORES`，必须能在 Linux 上编译并单测，
禁止出现 Win32 / X11 / Cocoa 引用。**新增共享核心要加进这个列表**（`menu_core.rs` 是最近一个）。

### 3.2 `editor_core` 是叶子模块

`editor_core` **不得**依赖 `syntax_core`（依赖方向是 `syntax_core` → `editor_core`）。

`tools/windows_editor_sim.py` 只链接 `text_buffer` + `editor_core`，所以**任何向上的依赖都会让它编译失败**。
我做括号配对时踩过：正确做法是**由调用方把掩码传进来**，不要为了让模拟器通过而放宽它。

### 3.3 文件行数上限

`src/windows_app/*.rs` 每文件 ≤1000 行，模块根 ≤1500 行，由 `tests/module_layout.rs` 强制。
超了要**拆代码**，不是放宽限制。

### 3.4 编码规则

- UI 状态进结构体字段；**绘制与命中测试共用同一个几何函数**（这条本会话修了两次）。
- 动画用 `motion_core` 的 `MotionRole`，不要每处一套计时器。
- 用户可见文案必须双语：`tr("English", "中文")`。
- 路径内部统一 `/`，平台边界才转换。

---

## 4. 本会话（DeepSeek）做了什么

### 4.1 第一批：菜单栏（`5a49013` 之前若干提交）

接手时菜单栏的真实状况比文档描述的更糟：**只有 3 个菜单有状态和命中测试**，
「运行」「调试」是画了边框和文字的死按钮（点了什么都不发生），「窗口」「帮助」根本不存在。
根因是每个点击处理器都手写一遍"关掉其他两个"的列表，所以后来加的菜单没人补进去。

- 新建共享模型 `src/menu_core.rs`（8 个菜单 / 43 个菜单项），**测试抓出我自己 3 个建模错误**：
  撤销/重做同时列在「文件」和「编辑」、停止同时列在「运行」和「调试」、一个项漏登记。
- Linux 菜单栏改为按模型绘制，8 个菜单全部可点，命中测试与绘制共用矩形。
- 一个 `open_menu` 字段取代三个布尔值。

### 4.2 顺手修掉的、推送前就存在的阻塞

这些在 main 上已经存在，会让验证永远红：

1. `src/tcide_project.rs` 一处 clippy 报错。
2. `src/windows_app/workspace.rs` 用了 `tcide_project` 却没导入，且 `language` 类型不匹配（`&str` → `String`）。
3. **`graph-callees` 渲染场景没真正生效**，产出帧与普通编辑器帧**逐字节相同**（都是 597119 字节）。
   我用 `git worktree` 检出动手前的提交核实过：**那时就已经相同**，所以 CI 一直是红的。
   根因是场景从「当前恢复的会话」取文档，而那是 `main.c`（6 行、0 个带接收者的调用），
   查找落空 → `prime_scene` 报「未知场景」→ 画出普通编辑器帧。**没有任何报错，唯一症状是文件大小相同。**
   现在场景自带文档并主动打开面板；修复后 667460 字节 vs 编辑器 600125 字节。

### 4.3 第二批（用户明确要求的 4 项，全部完成）

提交：`a268d67` → `03aa2e8` → `7451185` → `77ac329` → `8a381ab`

| 项 | 内容 | 测试 |
|---|---|---|
| 1 | **菜单键盘导航与悬停**：左右环绕切标题、上下在菜单内移动（到端点停住不回绕）、Enter 执行、Esc 关闭；指针与键盘共用一个光标 | 6 |
| 2 | **逐字输入/删除动画**：每个字符有自己的时刻，从下方升入并渐亮，取代「整行一起抬升」 | 5 |
| 3 | **动效曲线按角色统一**：新增 `MotionRole`（Overlay / Drawer / Follow），各自一套弹簧与行程 | 5 |
| 4 | **标签关闭/新建 + `.tcIDE` 进关系图 + 面板比例命名化** | 4 + 4 |

**第 4 项里有一处值得你知道**：项目树宽度原本是 `0.17` 在 **18 处**重复书写，现集中为 `motion_core::Layout`。
重构以**渲染帧逐像素不变**验证——证明只是换写法，没有移动列。

**动效调参前先量**：数学稳定阈值（0.0008）让每个动效看起来要 760ms，我差点据此去调本来没问题的弹簧。
人眼感知的是前 90%：overlay 实际只需 **150ms**、drawer 167ms、follow 133ms。

---

## 5. 现在的共享模型提供了什么（Codex 可以直接用）

### `src/menu_core.rs`

```rust
Menu::ALL                    // 8 个菜单，顺序即栏上顺序
Menu::title() / title_chinese() / index() / opens_panel() / items()
Item::ALL                    // 43 个菜单项
Item::label() / label_chinese() / shortcut() / menu()
MenuCursor { menu, item }    // 键盘位置
MenuCursor::moved(MenuMove)  // Left/Right 环绕，Up/Down 停住
MenuCursor::current_item()
menu_under(&[(Menu, x, y, w, h)], x, y) -> Option<Menu>
// 标签几何
tab_width(name_width, padding, minimum) -> i32
tab_close_width(inset) -> i32
tab_at(&[(left, width)], close_width, x) -> Option<TabHit>
selection_after_close(closed, count) -> Option<usize>
```

**关键语义**：左右**环绕**、上下**停住**（不回绕），切换标题高亮重置到第一项，
相邻按钮边界归**右侧**那个（否则边界像素命中不确定、指针划过时高亮会闪）。

### `src/motion_core.rs`

```rust
MotionRole::{Overlay, Drawer, Follow}   // 各自 profile() 与 travel()
arrival(role, value) -> (progress, offset, opacity)
character_progress(index, count, age, duration) -> f32
character_arrival(progress, travel) -> (offset, opacity)
Layout { workspace, graph, terminal }   // 列比例 + MIN_EDITOR
Layout::workspace_pixels / graph_pixels / terminal_pixels
```

---

## 6. 下一步（按优先级）

1. **把菜单语义同步到 Windows 后端**。已完成：Win32 原生菜单直接由
   `menu_core::Menu::ALL`/`Item` 生成，8 个顶级菜单与 Linux 共用顺序、中英文和快捷键；
   保存、构建/运行/停止、调试、扩展/设置/AI、终端及帮助等已连到真实动作。
   Windows 尚无对应视图状态的 7 项会明确置灰，不再伪装成可点击按钮；
   `--self-check` 验证每个可用项的 `WM_COMMAND` id 可逆。
2. **菜单子菜单展开动效**（未做）。
3. **注释气泡 / 关系线随焦点平滑出现消失**（未做）。
4. **拖动调整面板比例**时的吸附手感（`Layout` 已有比例，但没有拖动交互）。
5. **`.tcIDE` 与 AI 上下文联动**：AI 应优先读 `.tcIDE` 里的项目结构与关系，而不是重新猜测。

---

## 7. 明确没做、且不要假装做了的

- **正则搜索**：`search_core::Query` 有 `regex` 字段，但匹配**仍是字面量**。代码注释与文档都写明了。
- **Tree-sitter 级增量解析**：语义关系是「语言感知扫描 + 返回类型推导」，**不是真正的语法树**。
- **签名安装包**：流程与命令已实现，**需要用户自己的证书**；仓库不内置任何证书或密钥。
- **扩展的真实下载**：下载/校验/解包/安装计划都已实现，但**目录由用户提供**
  （`<项目>/.tc-ide-catalog.conf`），仓库不内置编造的 URL 与校验和。
- **macOS 的 Metal 渲染与直接 CoreText 排版**：本机无法验证，不要盲写。

---

## 8. 硬约束（不要违反）

- **不创建标签、不发 Release、不触发 `release.yml`**。它只有 `workflow_dispatch`，从未被触发。
  仓库现有 2 个标签是历史遗留，不要动。
- **不使用 WebView / HTML / CSS 运行时**。HTML 预览只用于设计确认，不进产品。
- **任何密钥不得出现在仓库、配置、日志、命令行**。AI Key 只走环境变量。
- **不执行破坏性 git 命令**。⚠️ 我在本会话用 `git checkout -- src/linux_app.rs` 清理失败编辑，
  把那一轮未提交的改动**全部冲掉**，只能重做。改用行范围精确剪除。
- **不要为通过测试而修改断言**。先核实真实行为再改代码——本会话有三次是我的断言写错、代码是对的。

---

## 9. 文档地图

| 文件 | 内容 |
|---|---|
| `docs/DEEPSEEK_HANDOFF.md` | 需求原文 + 本会话第一批进展 |
| `docs/ARCHITECTURE.md` | 分层、平台边界、关系图层级语义 |
| `docs/HANDOFF.md` | 需求队列（§5，权威清单）、方向变更（§11） |
| `docs/TAKEOVER.md` | 更早一版的接手说明（部分内容已被本文取代） |
| `docs/KNOWN-ISSUES.md` | Windows/Wine 配方、macOS SDK 限制 |
| `docs/REAL-MACHINE-CHECKLIST.md` | Windows 实机验收清单（给人执行） |
| `CHANGELOG.md` | 变更历史 |
| `tools/windows_selfcheck_wine.sh` | 在 Linux 上编译并自检 Windows 后端 |
| `tools/windows_editor_sim.py` | 模块边界 + 编辑序列守卫 |

---

## 10. 接手第一步（建议）

```bash
cd /home/tianchuangya/桌面/TC-IDE
git log --oneline -8
~/.cargo/bin/rustup run stable cargo test --all-targets   # 应为 352 + 3 + 4
~/.cargo/bin/rustup run stable cargo clippy --all-targets -- -D warnings
python3 tools/windows_editor_sim.py
# 然后读 §6，从「把菜单同步到 Windows」开始
```

**有任何不确定，先在代码里核实再写。** 本仓库的历史文档里出现过多处过期结论
（`docs/HANDOFF.md` 第 10 节曾把已交付的文本缓冲/LSP/DAP 列为缺失，后被逐项核对重写）。
**怀疑文档，去查代码。**

## 11. 本次验收与收尾（2026-10-17）

- 重新执行稳定 Rust 工具链的 `fmt --check`、`test --all-targets`、Clippy（Linux）和编辑器模拟器；当前结果为 **352 + 3 + 4 全部通过**，无 Clippy 警告。
- 实际导出并目视检查 `menu-hover`、`typing`、`project-document` 三个 Linux 原生渲染帧：中文菜单、代码编辑区、项目树、关系图、终端和 `.tcIDE` 关系文档均可见。
- Windows AI 请求现在会把当前项目的 `.tcIDE` 文档作为只读上下文附加到请求，最多 12KB，超长按 UTF-8 字符边界截断；聊天历史仍只保存用户原始问题，不保存密钥。
- Linux 面板拖动增加软吸附点（关系图 24/32/39/48%，终端 20/30/40/50%），拖动仍连续，接近常用比例时会稳定落位。
- Windows 项目同步会写入 `.tcIDE` 的主语言、版本和环境字段，不再只保存文件/符号关系。

### 当前仍需真实机器完成的验收

Windows 真机上的 DirectWrite/Direct2D、ConPTY、编译器/LSP/调试器和安装包签名，必须在 Windows 主机执行 `docs/REAL-MACHINE-CHECKLIST.md`；Linux 交叉编译只能证明目标代码可编译，不能替代硬件和 SDK 验收。macOS Metal/CoreText 同理。
