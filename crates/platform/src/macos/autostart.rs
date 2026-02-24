use anyhow::Result;
use crate::AutoStart;

/// macOS 开机自启动（基于 SMAppService / ServiceManagement）
pub struct MacAutoStart;

impl MacAutoStart {
    pub fn new() -> Self {
        Self
    }
}

impl AutoStart for MacAutoStart {
    fn is_enabled(&self) -> bool {
        unsafe {
            use objc::*;

            if let Some(cls) = objc::runtime::Class::get("SMAppService") {
                let service: *mut objc::runtime::Object = msg_send![cls, mainApp];
                if !service.is_null() {
                    let status: i64 = msg_send![service, status];
                    // SMAppServiceStatusEnabled = 1
                    return status == 1;
                }
            }

            false
        }
    }

    fn set_enabled(&self, enabled: bool) -> Result<()> {
        unsafe {
            use objc::*;

            if let Some(cls) = objc::runtime::Class::get("SMAppService") {
                let service: *mut objc::runtime::Object = msg_send![cls, mainApp];
                if !service.is_null() {
                    if enabled {
                        let mut error: *mut objc::runtime::Object = std::ptr::null_mut();
                        let _: () = msg_send![service, registerAndReturnError: &mut error];
                        if !error.is_null() {
                            log::warn!("启用登录项失败");
                        }
                    } else {
                        let mut error: *mut objc::runtime::Object = std::ptr::null_mut();
                        let _: () = msg_send![service, unregisterAndReturnError: &mut error];
                        if !error.is_null() {
                            log::warn!("禁用登录项失败");
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
