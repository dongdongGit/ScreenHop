#![allow(unexpected_cfgs)]

use anyhow::Result;
use screenhop_core::{MonitorInfo, Point, Rect};

/// 鼠标事件
#[derive(Debug, Clone)]
pub struct MouseEvent {
    /// 鼠标点击位置
    pub point: Point,
    /// 鼠标按键编号（2 = 中键）
    pub button: u32,
}

/// 窗口句柄（平台无关的包装）
#[derive(Debug, Clone)]
pub struct WindowHandle {
    #[cfg(target_os = "macos")]
    pub(crate) inner: macos::MacWindowHandle,
    #[cfg(target_os = "windows")]
    pub(crate) inner: windows::WinWindowHandle,
}

/// 鼠标钩子 trait
pub trait MouseHook {
    /// 启动鼠标钩子，接收中键点击回调
    fn start<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(MouseEvent) -> bool + Send + 'static;

    /// 停止鼠标钩子
    fn stop(&mut self) -> Result<()>;

    /// 钩子是否处于活跃状态
    fn is_active(&self) -> bool;
}

/// 窗口管理 trait
pub trait WindowManager {
    /// 获取指定位置的窗口
    fn get_window_at(&self, point: Point) -> Option<WindowHandle>;

    /// 获取窗口的 frame（位置 + 尺寸）
    fn get_window_frame(&self, handle: &WindowHandle) -> Option<Rect>;

    /// 设置窗口位置
    fn set_window_position(&self, handle: &WindowHandle, pos: Point) -> Result<()>;

    /// 设置窗口尺寸
    fn set_window_size(&self, handle: &WindowHandle, width: f64, height: f64) -> Result<()>;

    /// 激活窗口（置前 + 获取焦点）
    fn activate_window(&self, handle: &WindowHandle) -> Result<()>;

    /// 窗口是否处于最大化状态
    fn is_maximized(&self, handle: &WindowHandle) -> bool;

    /// 还原窗口
    fn restore_window(&self, handle: &WindowHandle) -> Result<()>;

    /// 最大化窗口
    fn maximize_window(&self, handle: &WindowHandle) -> Result<()>;
}

/// 命中检测 trait
pub trait HitTester {
    /// 判断点击位置是否在窗口标题栏上
    fn is_title_bar_hit(&self, handle: &WindowHandle, point: Point) -> bool;

    /// 判断点击位置是否在交互式标签页上（如浏览器标签、资源管理器标签）
    fn is_interactive_tab(&self, handle: &WindowHandle, point: Point) -> bool;
}

/// 显示器管理 trait
pub trait MonitorManager {
    /// 获取所有显示器信息
    fn get_monitors(&self) -> Vec<MonitorInfo>;

    /// 获取窗口所在的显示器
    fn get_monitor_for_window(&self, handle: &WindowHandle) -> Option<MonitorInfo>;
}

/// 开机自启动 trait
pub trait AutoStart {
    /// 是否已启用自启动
    fn is_enabled(&self) -> bool;

    /// 设置自启动状态
    fn set_enabled(&self, enabled: bool) -> Result<()>;
}

/// 权限检查 trait
pub trait PermissionChecker {
    /// 检查是否有必要的权限（macOS 辅助功能权限 / Windows 管理员权限）
    fn check_permissions(&self) -> bool;

    /// 请求权限（打开系统设置等）
    fn request_permissions(&self) -> Result<()>;
}

// 平台实现模块
#[cfg(target_os = "macos")]
pub mod macos;

#[cfg(target_os = "windows")]
pub mod windows;

// 平台工厂函数
#[cfg(target_os = "macos")]
pub fn create_platform() -> macos::MacPlatform {
    macos::MacPlatform::new()
}

#[cfg(target_os = "windows")]
pub fn create_platform() -> windows::WinPlatform {
    windows::WinPlatform::new()
}
