use anyhow::Result;
use screenhop_core::{Point, Rect};

use crate::{WindowHandle, WindowManager};
use super::WinWindowHandle;

use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::*;

/// Windows 窗口管理器（基于 Win32 API）
pub struct WinWindowManager;

impl WinWindowManager {
    pub fn new() -> Self {
        Self
    }
}

impl WindowManager for WinWindowManager {
    fn get_window_at(&self, point: Point) -> Option<WindowHandle> {
        unsafe {
            let pt = POINT {
                x: point.x as i32,
                y: point.y as i32,
            };
            let hwnd = WindowFromPoint(pt);
            if hwnd.0 == std::ptr::null_mut() {
                return None;
            }

            // 向上找到顶层窗口
            let mut top = hwnd;
            loop {
                let parent = GetAncestor(top, GA_ROOT);
                if parent.0 == std::ptr::null_mut() || parent == top {
                    break;
                }
                top = parent;
            }

            Some(WindowHandle {
                inner: WinWindowHandle { hwnd: top.0 as isize },
            })
        }
    }

    fn get_window_frame(&self, handle: &WindowHandle) -> Option<Rect> {
        unsafe {
            let hwnd = HWND(handle.inner.hwnd as *mut _);
            let mut rect = RECT::default();
            if GetWindowRect(hwnd, &mut rect).is_ok() {
                Some(Rect::new(
                    rect.left as f64,
                    rect.top as f64,
                    (rect.right - rect.left) as f64,
                    (rect.bottom - rect.top) as f64,
                ))
            } else {
                None
            }
        }
    }

    fn set_window_position(&self, handle: &WindowHandle, pos: Point) -> Result<()> {
        unsafe {
            let hwnd = HWND(handle.inner.hwnd as *mut _);
            let _ = SetWindowPos(
                hwnd,
                HWND_TOP,
                pos.x as i32,
                pos.y as i32,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_SHOWWINDOW,
            );
        }
        Ok(())
    }

    fn set_window_size(&self, handle: &WindowHandle, width: f64, height: f64) -> Result<()> {
        unsafe {
            let hwnd = HWND(handle.inner.hwnd as *mut _);
            let mut rect = RECT::default();
            let _ = GetWindowRect(hwnd, &mut rect);
            let _ = SetWindowPos(
                hwnd,
                HWND_TOP,
                rect.left,
                rect.top,
                width as i32,
                height as i32,
                SWP_NOMOVE | SWP_NOZORDER | SWP_SHOWWINDOW,
            );
        }
        Ok(())
    }

    fn activate_window(&self, handle: &WindowHandle) -> Result<()> {
        unsafe {
            let hwnd = HWND(handle.inner.hwnd as *mut _);
            let _ = SetForegroundWindow(hwnd);
        }
        Ok(())
    }

    fn is_maximized(&self, handle: &WindowHandle) -> bool {
        unsafe {
            let hwnd = HWND(handle.inner.hwnd as *mut _);
            IsZoomed(hwnd).as_bool()
        }
    }

    fn restore_window(&self, handle: &WindowHandle) -> Result<()> {
        unsafe {
            let hwnd = HWND(handle.inner.hwnd as *mut _);
            let _ = ShowWindow(hwnd, SW_RESTORE);
        }
        Ok(())
    }

    fn maximize_window(&self, handle: &WindowHandle) -> Result<()> {
        unsafe {
            let hwnd = HWND(handle.inner.hwnd as *mut _);
            let _ = ShowWindow(hwnd, SW_MAXIMIZE);
        }
        Ok(())
    }
}
