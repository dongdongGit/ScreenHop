<div align="center">

<img src="./assets/ScreenHopIconTray.svg" alt="ScreenHop Logo" width="128" />

# ScreenHop

**一键跨屏，窗口无缝穿梭**<br>
利用鼠标中键点击标题栏，让窗口瞬间跳转到下一个显示器！

[![Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-macOS%20%7C%20Windows-lightgrey.svg)](#-安装)

[下载最新版本](https://github.com/dongdongGit/ScreenHop/releases/latest) • [问题反馈](https://github.com/dongdongGit/ScreenHop/issues) • [源码构建](#%EF%B8%8F-从源码构建)

---

</div>

## ✨ 核心功能

- 🖱️ **中键一击即飞** — 只需在窗口标题栏按下鼠标中键，窗口立刻飞跃到下一个显示器。
- 📐 **智能比例等比缩放** — 自动计算两块屏幕分辨率差异，保持窗口相对新屏幕的比例与位置不变。
- 📏 **超大窗口智能适配** — 如果目标显示器较小，窗口会自动缩放适配，避免超出屏幕边界。
- 🚫 **标签页防误触保护** — 智能识别浏览器（Chrome/Edge等）及资源管理器的标签页区域，避免在切换标签时不小心移动整个窗口。
- 🔄 **开机自启与静默运行** — 支持随系统开机自启，隐藏在系统托盘，不打扰你的日常工作。
- 🌍 **内置代理支持** — 提供代理设置（支持 HTTP/HTTPS/SOCKS4/SOCKS5 及密码认证）以稳定获取更新。
- 🔔 **自动更新检测** — 连接 GitHub Release 自动检查并提醒新版本，确保始终使用最新版。

## 📥 安装

前往 [GitHub Releases 页面](https://github.com/dongdongGit/ScreenHop/releases) 下载最新版本。

### 🍎 macOS 用户

1. 下载并解压最新的 `ScreenHop-macOS-universal.zip`。
2. 将 `ScreenHop` 应用移动到您的应用程序文件夹。
3. 双击运行。
4. **⚠️ 首次运行权限要求**：<br>
   ScreenHop 需要监听您的鼠标中键事件以控制窗口，系统会请求**辅助功能**权限。<br>
   前往：`系统设置` → `隐私与安全性` → `辅助功能`，勾选 `ScreenHop`。

### 🪟 Windows 用户

1. 下载最新的 `ScreenHop-win-x64.zip`。
2. 解压后将整个文件夹放在您喜欢的位置。
3. 运行 `screenhop.exe`，可在托盘图标右键菜单中设置“开机自动启动”。

## ⚙️ 托盘设置

右键点击系统托盘图标可进行相关配置：
- **启用/禁用功能**：随时暂停或恢复鼠标中键跳转功能。
- **开机自动启动**：一键设置随系统启动。
- **检查更新**：手动检查最新版本。
- **代理设置**：配置代理服务器地址（支持验证），彻底解决国内访问 GitHub 更新过慢或失败的问题。

## 🏗️ 从源码构建

本项目基于 Rust 编写，通过 Cargo 构建。支持跨平台交叉编译。

```bash
# 1. 安装 Rust 工具链
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 2. 克隆项目
git clone https://github.com/dongdongGit/ScreenHop.git
cd ScreenHop

# 3. 编译发布版本
cargo build --release
```

构建完成后，可执行文件将生成在：
* `target/release/screenhop` (macOS/Linux)
* `target/release/screenhop.exe` (Windows)

## 📁 架构设计

ScreenHop 将核心逻辑与各具体平台的系统 API 调用进行了分离，方便未来扩展和维护：

```text
crates/
├── core/       => 跨平台共享核心（配置读写、显示器几何数学计算、版本更新调度）
├── platform/   => OS 系统接口抽象与具体实现
│   ├── macos/    -> 使用 CGEventTap 拦截鼠标，AXUIElement 移动缩放窗口
│   └── windows/  -> 使用 WH_MOUSE_LL 全局钩子，Win32 API 操作窗口句柄
└── app/        => 主程序入口、系统托盘菜单 (tray-icon UI)、应用状态管理
```

## 🤝 鸣谢与参与

欢迎提交 Issue 和 Pull Request，我们非常高兴能与你一起改进 ScreenHop。如果你觉得这个小工具有用，请在 GitHub 上点一个 ⭐ 鼓励一下！

## 📄 许可证

本项目基于 [MIT License](LICENSE) 协议开源。
