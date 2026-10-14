# 安装、升级与卸载

TC-IDE 目前处于开发阶段，尚未发布正式版本，也没有创建任何版本标签或 GitHub Release。
下面说明的是各平台真实的安装方式；在项目所有者明确说"正式开始"之前，不会执行发布流程。

## 从源码构建

```bash
# Linux：需要原生图形库
sudo apt install build-essential libx11-dev libcairo2-dev libpango1.0-dev
cargo build --release

# Windows（PowerShell）
cargo build --release

# macOS
cargo build --release
```

运行：`target/release/tc-ide`（Windows 为 `target\release\tc-ide.exe`）。
调试用参数：

- `tc-ide --render-frame <path>`（Linux）：不映射窗口渲染一帧到 PNG，用于视觉检查与 CI 冒烟测试。
- `tc-ide --self-check`（Windows）：运行状态层自检并打印结果，失败时返回非零退出码。

## Linux

发布工作流在 Ubuntu runner 上生成三种产物，均可在本机安装或直接运行：

- **`.deb`**（`dpkg-deb` 生成）：

  ```bash
  sudo apt install ./tc-ide_<版本>_amd64.deb
  # 升级
  sudo apt install ./tc-ide_<新版本>_amd64.deb
  # 卸载
  sudo apt remove tc-ide
  ```

  依赖：`libx11-6`、`libcairo2`、`libpango-1.0-0`。

- **`.tar.gz`**（便携）：解压后直接运行 `tc-ide`，无需安装；删除目录即完成卸载。

- **AppImage**（可选）：`chmod +x` 后直接运行。AppImage 工具链需要联网下载，
  若下载失败工作流会保留说明，不会生成伪造的产物。

## Windows

- **ZIP**：解压后运行 `tc-ide.exe`；删除目录即完成卸载。
- **MSIX**：仅在安装了你自己的代码签名证书后才可安装（`Add-AppxPackage`）。
  未签名包需要开发者模式或先签名：

  ```powershell
  # 在 TC-IDE 的打包面板中选择证书后打包，或手动签名
  signtool sign /fd SHA256 /f <证书.pfx> .tc-ide-packages\<项目>.msix
  Add-AppxPackage .tc-ide-packages\<项目>.msix
  # 卸载
  Remove-AppxPackage <包名>
  ```

  TC-IDE 不会生成、上传或保存私钥与密码；受密码保护的证书请在终端中手动签名。

## macOS

- **`.app` / `.dmg`**：将 `TC-IDE.app` 拖入"应用程序"。
  未签名/未公证的应用首次打开需要在"系统设置 → 隐私与安全性"中允许。
- 卸载：把 `TC-IDE.app` 移到废纸篓。

## 配置与数据位置

| 内容 | Windows | Linux / macOS |
| --- | --- | --- |
| 设置（主题、语言、字体、背景、AI 端点/模型） | `%APPDATA%\TC-IDE\settings.conf` | Linux：`~/.config/tc-ide/*.conf` |
| 会话（项目、打开的文件、最近项目） | `%APPDATA%\TC-IDE\session.conf` | 尚未实现 |
| 运行时缓存清单 | `%LOCALAPPDATA%\TC-IDE\runtimes\cache.conf` | 尚未实现 |
| 扩展目录（用户提供） | `<项目>/.tc-ide-catalog.conf` | 同左 |

设置文件中永远不会写入 API Key：Key 只在进程内存中，并通过子进程环境变量传给终端里的请求。
