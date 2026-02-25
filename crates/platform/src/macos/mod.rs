pub mod hook;
pub mod window;
pub mod monitor;
pub mod hittest;
pub mod autostart;


/// macOS 平台窗口句柄（AXUIElement 的包装）
#[derive(Debug, Clone)]
pub struct MacWindowHandle {
    /// AXUIElement 的原始指针 (保持引用安全)
    pub(crate) ax_element: *const std::ffi::c_void,
    /// 窗口所属进程 PID
    pub(crate) pid: i32,
}

// AXUIElement 是线程安全的
unsafe impl Send for MacWindowHandle {}
unsafe impl Sync for MacWindowHandle {}

/// macOS 平台实现集合
pub struct MacPlatform {
    pub hook: hook::MacMouseHook,
    pub window_manager: window::MacWindowManager,
    pub hit_tester: hittest::MacHitTester,
    pub monitor_manager: monitor::MacMonitorManager,
    pub auto_start: autostart::MacAutoStart,
}

impl MacPlatform {
    pub fn new() -> Self {
        Self {
            hook: hook::MacMouseHook::new(),
            window_manager: window::MacWindowManager::new(),
            hit_tester: hittest::MacHitTester::new(),
            monitor_manager: monitor::MacMonitorManager::new(),
            auto_start: autostart::MacAutoStart::new(),
        }
    }

    /// 静默检查是否已授予辅助功能权限（不弹出系统提示框）
    pub fn check_accessibility_permissions(&self) -> bool {
        unsafe {
            extern "C" {
                fn AXIsProcessTrusted() -> bool;
            }
            AXIsProcessTrusted()
        }
    }

    /// 请求辅助功能权限（会弹出系统提示框引导用户授权）
    pub fn request_accessibility_permissions(&self) {
        use core_foundation::base::TCFType;
        use core_foundation::boolean::CFBoolean;
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::string::CFString;

        unsafe {
            let key = CFString::new("AXTrustedCheckOptionPrompt");
            let value = CFBoolean::true_value();
            let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);

            extern "C" {
                fn AXIsProcessTrustedWithOptions(options: core_foundation::base::CFTypeRef) -> bool;
            }

            // 调用时传入 Prompt=true，macOS 会自动弹出授权引导对话框
            AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef() as _);
        }
    }
}
