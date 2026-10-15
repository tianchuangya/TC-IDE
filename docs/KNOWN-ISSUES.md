# 已知问题

本文件只记录**当前代码真实存在**的限制，避免把"计划中"写成"已完成"。
TC-IDE 尚未发布正式版本，也尚未完成 Windows 实机验收。

## 必须实机确认（没有 Windows 设备无法完成）

- 高 DPI、多显示器、窗口最大化/缩放下的布局与文字清晰度。
- 中文 IME 在编辑器、弹出终端、AI 提问框中的候选窗与组合行为。
- 系统剪贴板在拖选、`Ctrl+Shift+C`、粘贴大段文本时的表现。
- 弹出终端的滚动、选择、复制与关闭顺序；关闭 IDE 后是否残留 `powershell.exe`。
- Build/Run、ZIP/MSIX 打包在真实项目上的耗时与失败信息可读性。
- 新增的关系图折叠、标签页、脏状态确认、AI 请求与格式化/调试动作。
- 代码字形宽度已改为 DirectWrite 实测，但**尚未在真机对比**光标与选区是否完全对齐。

## 功能缺口

- **编辑器文本模型**：已是 piece table，并且只取出可见行绘制，项目树也有滚动条；
  语义关系仍是正则与启发式（`index_core` 让索引增量化，但没有真正的语法树解析）。
- **LSP**：客户端已在 Windows 侧运行（补全、诊断、跳转定义、悬停、引用、重命名、带严重级别过滤的问题面板）；
  仍缺重命名预览。
- **调试**：已有真实 DAP 会话，调用栈/变量/断点显示在问题面板中并可回车跳转；
  仍缺断点的就地增删与条件断点，且只在 Windows 侧接入。
- **语法分析**：`syntax_core` 已用语言感知的作用域扫描替换行正则与括号计数（声明归属、调用归属、类成员）；但它不是完整的语法树，仍无表达式/类型分析，跨文件关系也只按声明名匹配。
- **扩展安装**：下载、校验、解包、卸载命令、运行时缓存清单与按语言的真实工具状态都已实现；
  但目录需要用户自己提供（`<项目>/.tc-ide-catalog.conf`），仓库不会内置编造的 URL 与校验和。
- **AI**：面板、流式输出、超时、取消、错误说明、多轮上下文与会话历史持久化均已接入；
  仍缺多会话切换与消息复制。
- **macOS**：Cocoa 原生窗口、编辑会话、标签、剪贴板、打开面板、PTY 终端、关系图、设置与工具链面板已经接入；仍缺 Metal 渲染、直接 CoreText 排版、打包面板与真机观感确认。
- **Linux**：没有会话恢复与运行时缓存清单；`linux_app.rs` 仍偏大。Windows 后端已经拆分为入口和 14 个子模块。
- **关系图**：已有后台增量索引、视口裁剪、节点预算与文件夹聚类；仍缺少按缩放切换的层级细节。
- **视觉细节**：悬停高亮、完整中文文案、统一弹簧模型（`motion_core`）、文件树行位置弹簧、多字符编辑动画以及 Linux/Windows 背景裁剪预览均已完成；Windows 设置与扩展浮层仍缺完整的禁用态和键盘焦点环，最终 DPI 命中观感仍需实机确认。
- **ConPTY 网格**：面板关闭时仍按完全展开的几何上报行列，属有意为之（避免 shell 看到 1 行）。

## 工程约束

- Windows 代码只能在 Linux 上做**类型检查**（`cargo check --target x86_64-pc-windows-msvc`）
  与 Clippy，不能替代真机行为验证。
- 发布工作流只能手动触发；在项目所有者说"正式开始"之前不会创建标签或 Release。
- AppImage 步骤依赖联网下载 appimagetool，失败时不会阻断发布，但产物会缺失（文件清单会说明）。

## 已在本地复现 Windows 测试（重要方法）

本项目此前无法在本机验证 Windows 行为，只能靠 CI 猜。现在可以了：

```bash
# 1. 一次性地把 mingw 运行时与 binutils 取下来（无需 root）
mkdir -p /tmp/mingw && cd /tmp/mingw
apt-get download gcc-mingw-w64-x86-64-win32 gcc-mingw-w64-x86-64-win32-runtime \
                 mingw-w64-x86-64-dev binutils-mingw-w64-x86-64
for f in *.deb; do dpkg-deb -x "$f" root/; done

# 2. 让 cargo 用 rust-lld 加这些库来链接 Windows 目标
#    （.cargo/config.toml）
#    [target.x86_64-pc-windows-gnu]
#    linker = "rust-lld"
#    rustflags = ["-C","link-self-contained=yes",
#                 "-L","native=/tmp/mingw/root/usr/x86_64-w64-mingw32/lib",
#                 "-L","native=/tmp/mingw/root/usr/lib/gcc/x86_64-w64-mingw32/13-win32"]

# 3. 编译并在 Wine 里跑真正的 Windows 测试二进制
export PATH="/tmp/mingw/root/usr/bin:$PATH"
cargo test --target x86_64-pc-windows-gnu --no-run
wine target/x86_64-pc-windows-gnu/debug/deps/tc_ide-*.exe --test-threads=4
```

要点：
- 需要把 `src/main.rs` 换成 `src/lib.rs`（去掉各平台的 `fn main`、去掉 `compile_error!`、模块改成 `pub mod`），
  并把 `windows_app` 的 `#[cfg(target_os = "windows")]` 去掉，才能在 Linux 上编出 Windows 后端。
- 还要拷贝 `assets/`（图标被 `include_bytes!` 引用）。
- 这套东西跑 **240 项**测试（Linux 上是 255 项），多出来的正是 **5 个只在 Windows 存在的 `windows_app` 测试**——
  返回类型推断的 panic（`a < b > c` 被当成泛型，`rfind('>')` 落在 `<` 之前）就是这样被抓到的。
