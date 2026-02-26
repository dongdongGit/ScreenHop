use anyhow::Result;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use crate::MouseEvent;
use screenhop_core::Point;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::*;

/// Windows 鼠标钩子实现（基于 WH_MOUSE_LL）
pub struct WinMouseHook {
    active: Arc<AtomicBool>,
}

type CallbackType = Box<dyn Fn(MouseEvent) -> bool + Send + Sync>;

// 由于 HHOOK 底层包含原生指针，没有实现 Send / Sync，我们需要用一个 Wrapper 来包裹它
#[derive(Clone, Copy)]
struct HookHandle(HHOOK);

// 此时我们明确这是安全的，因为钩子句柄只会被用于卸载钩子和调用下一个钩子
unsafe impl Send for HookHandle {}
unsafe impl Sync for HookHandle {}

// 使用 OnceLock 来安全地存储全局回调和钩子句柄
fn global_callback() -> &'static Mutex<Option<CallbackType>> {
    static CALLBACK: OnceLock<Mutex<Option<CallbackType>>> = OnceLock::new();
    CALLBACK.get_or_init(|| Mutex::new(None))
}

fn hook_handle() -> &'static Mutex<Option<HookHandle>> {
    static HANDLE: OnceLock<Mutex<Option<HookHandle>>> = OnceLock::new();
    HANDLE.get_or_init(|| Mutex::new(None))
}

#[allow(static_mut_refs)]
impl Default for WinMouseHook {
    fn default() -> Self {
        Self::new()
    }
}

impl WinMouseHook {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 安装 WH_MOUSE_LL 钩子
    pub fn install_hook<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(MouseEvent) -> bool + Send + Sync + 'static,
    {
        if let Ok(mut cb) = global_callback().lock() {
            *cb = Some(Box::new(callback));
        }

        unsafe {
            let h_instance = GetModuleHandleW(None)?;
            let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(low_level_mouse_proc), h_instance, 0)?;

            if let Ok(mut handle) = hook_handle().lock() {
                *handle = Some(HookHandle(hook));
            }
        }

        self.active.store(true, Ordering::SeqCst);
        log::info!("WH_MOUSE_LL 鼠标钩子已安装");
        Ok(())
    }

    /// 卸载钩子
    pub fn uninstall_hook(&mut self) -> Result<()> {
        unsafe {
            if let Ok(mut handle) = hook_handle().lock() {
                if let Some(hook) = handle.take() {
                    let _ = UnhookWindowsHookEx(hook.0);
                }
            }
        }
        if let Ok(mut cb) = global_callback().lock() {
            *cb = None;
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
    let mut handled = false;

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

            if let Ok(cb_guard) = global_callback().lock() {
                if let Some(ref callback) = *cb_guard {
                    if callback(event) {
                        handled = true;
                    }
                }
            }
        }
    }

    if handled {
        // 事件已消费
        return LRESULT(1);
    }

    let handle = if let Ok(guard) = hook_handle().lock() {
        if let Some(h) = *guard {
            h.0
        } else {
            HHOOK::default()
        }
    } else {
        HHOOK::default()
    };

    CallNextHookEx(handle, n_code, w_param, l_param)
}
