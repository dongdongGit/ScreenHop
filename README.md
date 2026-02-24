# ScreenHop

> 中键点击标题栏，一键将窗口跳转到下一个显示器。

**ScreenHop** 是一个跨平台的多显示器窗口快速移动工具，使用 Rust 构建，支持 macOS 和 Windows。

## ✨ 功能

- 🖱️ **中键点击移动** — 在窗口标题栏按下鼠标中键，窗口自动跳到下一个显示器
- 📐 **智能定位** — 自动计算相对位置，保持窗口在目标显示器上的比例不变
- 📏 **尺寸适配** — 如果目标显示器更小，自动缩放窗口以适应
- 🚫 **标签页保护** — 智能识别浏览器/资源管理器标签页，避免误触
- 🔄 **开机自启** — 支持设置开机自动启动
- 🔔 **更新检查** — 自动检查 GitHub Release 新版本

## 📥 安装

从 [GitHub Releases](https://github.com/dongdongGit/ScreenHop/releases) 下载对应平台的压缩包：

### macOS
```bash
# 解压后运行
./screenhop
# 首次运行需授权：系统设置 → 隐私与安全性 → 辅助功能
```

### Windows
```
解压后运行 screenhop.exe
```

## 🏗️ 从源码构建

```bash
# 安装 Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 克隆并构建
git clone https://github.com/dongdongGit/ScreenHop.git
cd ScreenHop
cargo build --release
```

构建产物位于 `target/release/screenhop`（macOS）或 `target/release/screenhop.exe`（Windows）。

## 📁 项目结构

```
crates/
├── core/       跨平台共享逻辑（配置、显示器计算、更新检查）
├── platform/   平台抽象 + 具体实现
│   ├── macos/    CGEventTap + AXUIElement
│   └── windows/  WH_MOUSE_LL + Win32 API
└── app/        主程序入口 + 系统托盘
```

## 📄 License

MIT
