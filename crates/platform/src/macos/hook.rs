use anyhow::Result;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::MouseEvent;
use screenhop_core::Point;

/// macOS 鼠标钩子实现（基于 CGEventTap）
///
/// 使用 CGEventTapCreate 创建一个拦截 otherMouseDown 事件的 tap，
/// 绑定到当前线程的 CFRunLoop 中运行。
pub struct MacMouseHook {
    active: Arc<AtomicBool>,
}

// 全局回调存储，因为 CGEventTap 的 C 回调不支持闭包捕获
static mut GLOBAL_CALLBACK: Option<Box<dyn Fn(MouseEvent) -> bool + Send>> = None;

impl MacMouseHook {
    pub fn new() -> Self {
        Self {
            active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// 安装 CGEventTap 到当前线程的 RunLoop
    /// 必须在主线程调用（NSApp.run() 之前）
    pub fn install_event_tap<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(MouseEvent) -> bool + Send + 'static,
    {
        unsafe {
            GLOBAL_CALLBACK = Some(Box::new(callback));
        }

        self.active.store(true, Ordering::SeqCst);

        unsafe {
            // CGEventTap 类型定义
            type CGEventTapCallBack = unsafe extern "C" fn(
                proxy: *const std::ffi::c_void,
                event_type: u32,
                event: *const std::ffi::c_void,
                user_info: *mut std::ffi::c_void,
            ) -> *const std::ffi::c_void;

            extern "C" {
                fn CGEventTapCreate(
                    tap: u32,       // CGEventTapLocation
                    place: u32,     // CGEventTapPlacement
                    options: u32,   // CGEventTapOptions
                    events_of_interest: u64, // CGEventMask
                    callback: CGEventTapCallBack,
                    user_info: *mut std::ffi::c_void,
                ) -> *const std::ffi::c_void; // CFMachPortRef

                fn CFMachPortCreateRunLoopSource(
                    allocator: *const std::ffi::c_void,
                    port: *const std::ffi::c_void,
                    order: i64,
                ) -> *const std::ffi::c_void;

                fn CFRunLoopGetCurrent() -> *const std::ffi::c_void;

                fn CFRunLoopAddSource(
                    rl: *const std::ffi::c_void,
                    source: *const std::ffi::c_void,
                    mode: *const std::ffi::c_void,
                );

                fn CGEventTapEnable(tap: *const std::ffi::c_void, enable: bool);
            }

            // kCGEventOtherMouseDown = 25, kCGEventOtherMouseUp = 26
            // 事件掩码: (1 << 25) | (1 << 26)
            let event_mask: u64 = (1 << 25) | (1 << 26);

            let tap = CGEventTapCreate(
                0, // kCGHIDEventTap
                0, // kCGHeadInsertEventTap
                0, // kCGEventTapOptionDefault (active tap, can modify/consume)
                event_mask,
                event_tap_callback,
                std::ptr::null_mut(),
            );

            if tap.is_null() {
                anyhow::bail!("CGEventTapCreate 失败 — 请确认已授予辅助功能权限");
            }

            // 创建 RunLoop Source
            let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
            if source.is_null() {
                anyhow::bail!("CFMachPortCreateRunLoopSource 失败");
            }

            // 添加到当前 RunLoop
            let run_loop = CFRunLoopGetCurrent();

            // kCFRunLoopCommonModes
            extern "C" {
                static kCFRunLoopCommonModes: *const std::ffi::c_void;
            }

            CFRunLoopAddSource(run_loop, source, kCFRunLoopCommonModes);
            CGEventTapEnable(tap, true);

            log::info!("CGEventTap 已安装到 RunLoop");
        }

        Ok(())
    }
}

/// CGEventTap 的 C 回调函数
unsafe extern "C" fn event_tap_callback(
    _proxy: *const std::ffi::c_void,
    event_type: u32,
    event: *const std::ffi::c_void,
    _user_info: *mut std::ffi::c_void,
) -> *const std::ffi::c_void {
    // 处理 tap 被系统禁用的情况（比如系统过于繁忙）
    // kCGEventTapDisabledByTimeout = 0xFFFFFFFE
    // kCGEventTapDisabledByUserInput = 0xFFFFFFFF
    if event_type == 0xFFFFFFFE || event_type == 0xFFFFFFFF {
        log::warn!("CGEventTap 被系统禁用, 类型={}", event_type);
        return event;
    }

    // 只处理 otherMouseDown (25)
    if event_type != 25 {
        return event;
    }

    // 从 CGEvent 读取按键编号和位置
    extern "C" {
        fn CGEventGetIntegerValueField(event: *const std::ffi::c_void, field: u32) -> i64;
        fn CGEventGetLocation(event: *const std::ffi::c_void) -> core_graphics::geometry::CGPoint;
    }

    // kCGMouseEventButtonNumber = 3
    let button_number = CGEventGetIntegerValueField(event, 3);

    // 只处理中键 (button 2)
    if button_number != 2 {
        return event;
    }

    let location = CGEventGetLocation(event);

    let mouse_event = MouseEvent {
        point: Point {
            x: location.x,
            y: location.y,
        },
        button: 2,
    };

    // 调用全局回调
    if let Some(ref callback) = GLOBAL_CALLBACK {
        if callback(mouse_event) {
            // 事件已消费，返回 null 以吞掉该事件
            return std::ptr::null();
        }
    }

    // 传递事件
    event
}
