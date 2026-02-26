pub mod hook;
pub mod window;
pub mod monitor;
pub mod hittest;
pub mod autostart;

/// Windows 平台窗口句柄（HWND 的包装）
#[derive(Debug, Clone)]
pub struct WinWindowHandle {
    pub(crate) hwnd: isize, // HWND as raw pointer value
}

/// Windows 平台实现集合
pub struct WinPlatform {
    pub hook: hook::WinMouseHook,
    pub window_manager: window::WinWindowManager,
    pub hit_tester: hittest::WinHitTester,
    pub monitor_manager: monitor::WinMonitorManager,
    pub auto_start: autostart::WinAutoStart,
}

impl Default for WinPlatform {
    fn default() -> Self {
        Self::new()
    }
}

impl WinPlatform {
    pub fn new() -> Self {
        Self {
            hook: hook::WinMouseHook::new(),
            window_manager: window::WinWindowManager::new(),
            hit_tester: hittest::WinHitTester::new(),
            monitor_manager: monitor::WinMonitorManager::new(),
            auto_start: autostart::WinAutoStart::new(),
        }
    }
}
