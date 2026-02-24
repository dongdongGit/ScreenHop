#![allow(deprecated)] // cocoa crate fields are deprecated in favor of objc2-foundation

use screenhop_core::MonitorInfo;
use screenhop_core::Rect;

use crate::{MonitorManager, WindowHandle};

/// macOS 显示器管理器（基于 NSScreen）
pub struct MacMonitorManager;

impl MacMonitorManager {
    pub fn new() -> Self {
        Self
    }
}

impl MonitorManager for MacMonitorManager {
    fn get_monitors(&self) -> Vec<MonitorInfo> {
        unsafe {
            use cocoa::foundation::NSUInteger;
            use objc::*;

            let screens: *mut objc::runtime::Object = msg_send![class!(NSScreen), screens];
            let count: NSUInteger = msg_send![screens, count];

            if count == 0 {
                return vec![];
            }

            // 获取主屏幕高度用于坐标转换
            // NSScreen 坐标原点在左下角，Quartz/AX 坐标原点在左上角
            let main_screen: *mut objc::runtime::Object =
                msg_send![screens, objectAtIndex: 0usize];
            let main_frame: cocoa::foundation::NSRect = msg_send![main_screen, frame];
            let global_height = main_frame.size.height;

            let mut monitors = Vec::with_capacity(count as usize);

            for i in 0..count {
                let screen: *mut objc::runtime::Object = msg_send![screens, objectAtIndex: i];

                let frame: cocoa::foundation::NSRect = msg_send![screen, frame];
                let visible_frame: cocoa::foundation::NSRect = msg_send![screen, visibleFrame];

                // 转换为 Quartz 坐标系（左上角原点）
                let bounds = Rect::new(
                    frame.origin.x,
                    global_height - (frame.origin.y + frame.size.height),
                    frame.size.width,
                    frame.size.height,
                );

                let work_area = Rect::new(
                    visible_frame.origin.x,
                    global_height - (visible_frame.origin.y + visible_frame.size.height),
                    visible_frame.size.width,
                    visible_frame.size.height,
                );

                monitors.push(MonitorInfo {
                    id: i as u64,
                    bounds,
                    work_area,
                });
            }

            monitors
        }
    }

    fn get_monitor_for_window(&self, handle: &WindowHandle) -> Option<MonitorInfo> {
        use crate::WindowManager;
        let wm = super::window::MacWindowManager::new();
        let frame = wm.get_window_frame(handle)?;
        let center = screenhop_core::Point {
            x: frame.mid_x(),
            y: frame.mid_y(),
        };

        let monitors = self.get_monitors();
        screenhop_core::monitor::find_monitor_for_point(center, &monitors)
            .map(|idx| monitors[idx].clone())
    }
}
