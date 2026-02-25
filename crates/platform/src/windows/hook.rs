use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use crate::MouseEvent;
use screenhop_core::Point;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;

/// Windows 鼠标钩子实现（基于 WH_MOUSE_LL）
pub struct WinMouseHook {
    active: Arc<AtomicBool>,
}

static mut GLOBAL_CALLBACK: Option<Box<dyn Fn(MouseEvent) -> bool + Send>> = None;
static mut HOOK_HANDLE: Option<HHOOK> = None;

impl WinMouseHook {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 安装 WH_MOUSE_LL 钩子
    pub fn install_hook<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(MouseEvent) -> bool + Send + 'static,
    {
        unsafe {
            GLOBAL_CALLBACK = Some(Box::new(callback));

            let h_instance = GetModuleHandleW(None)?;
            let hook = SetWindowsHookExW(
                WH_MOUSE_LL,
                Some(low_level_mouse_proc),
                h_instance,
                0,
            )?;

            HOOK_HANDLE = Some(hook);
        }

        self.active.store(true, Ordering::SeqCst);
        log::info!("WH_MOUSE_LL 鼠标钩子已安装");
        Ok(())
    }

    /// 卸载钩子
    pub fn uninstall_hook(&mut self) -> Result<()> {
        unsafe {
            if let Some(hook) = HOOK_HANDLE.take() {
                let _ = UnhookWindowsHookEx(hook);
            }
            GLOBAL_CALLBACK = None;
        }
        self.active.store(false, Ordering::SeqCst);
        log::info!("WH_MOUSE_LL 鼠标钩子已卸载");
        Ok(())
    }
}

/// WH_MOUSE_LL 回调函数
unsafe extern "system" fn low_level_mouse_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let msg = w_param.0 as u32;
        // WM_MBUTTONDOWN = 0x0207
        if msg == 0x0207 {
            let mouse_struct = &*(l_param.0 as *const MSLLHOOKSTRUCT);
            let point = Point {
                x: mouse_struct.pt.x as f64,
                y: mouse_struct.pt.y as f64,
            };

            let event = MouseEvent {
                point,
                button: 2, // 中键
            };

            if let Some(ref callback) = GLOBAL_CALLBACK {
                if callback(event) {
                    // 事件已消费
                    return LRESULT(1);
                }
            }
        }
    }

    CallNextHookEx(HOOK_HANDLE.unwrap_or_default(), n_code, w_param, l_param)
}
