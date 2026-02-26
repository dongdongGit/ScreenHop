use screenhop_core::{MonitorInfo, Rect};

use crate::{MonitorManager, WindowHandle, WindowManager};

use windows::Win32::Foundation::{BOOL, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::*;

/// Windows 显示器管理器（基于 EnumDisplayMonitors）
pub struct WinMonitorManager;

impl Default for WinMonitorManager {
    fn default() -> Self {
        Self::new()
    }
}

impl WinMonitorManager {
    pub fn new() -> Self {
        Self
    }
}

impl MonitorManager for WinMonitorManager {
    fn get_monitors(&self) -> Vec<MonitorInfo> {
        let mut monitors = Vec::new();

        unsafe {
            let data = LPARAM(&mut monitors as *mut Vec<MonitorInfo> as isize);
            let _ = EnumDisplayMonitors(None, None, Some(enum_monitor_proc), data);
        }

        monitors
    }

    fn get_monitor_for_window(&self, handle: &WindowHandle) -> Option<MonitorInfo> {
        let wm = super::window::WinWindowManager::new();
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

unsafe extern "system" fn enum_monitor_proc(
    hmonitor: HMONITOR,
    _hdc: HDC,
    _lprc_clip: *mut RECT,
    data: LPARAM,
) -> BOOL {
    let monitors = &mut *(data.0 as *mut Vec<MonitorInfo>);

    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };

    if GetMonitorInfoW(hmonitor, &mut info).as_bool() {
        let bounds = Rect::new(
            info.rcMonitor.left as f64,
            info.rcMonitor.top as f64,
            (info.rcMonitor.right - info.rcMonitor.left) as f64,
            (info.rcMonitor.bottom - info.rcMonitor.top) as f64,
        );

        let work_area = Rect::new(
            info.rcWork.left as f64,
            info.rcWork.top as f64,
            (info.rcWork.right - info.rcWork.left) as f64,
            (info.rcWork.bottom - info.rcWork.top) as f64,
        );

        monitors.push(MonitorInfo {
            id: hmonitor.0 as u64,
            bounds,
            work_area,
        });
    }

    BOOL(1) // 继续枚举
}
