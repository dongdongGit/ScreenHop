use screenhop_core::Point;

use crate::{HitTester, WindowHandle};

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_NCHITTEST: u32 = 0x0084;
const HTCAPTION: i32 = 2;

/// Windows 命中检测器（基于 WM_NCHITTEST + UI Automation）
pub struct WinHitTester;

impl Default for WinHitTester {
    fn default() -> Self {
        Self::new()
    }
}

impl WinHitTester {
    pub fn new() -> Self {
        Self
    }
}

impl HitTester for WinHitTester {
    fn is_title_bar_hit(&self, handle: &WindowHandle, point: Point) -> bool {
        unsafe {
            let hwnd = HWND(handle.inner.hwnd as *mut _);

            // 使用 WM_NCHITTEST 判断是否在标题栏
            // MAKELPARAM: low word = x, high word = y
            let x = (point.x as i32) as u16 as u32;
            let y = (point.y as i32) as u16 as u32;
            let lparam = LPARAM(((y << 16) | x) as isize);
            let result = SendMessageW(hwnd, WM_NCHITTEST, WPARAM(0), lparam);

            if result.0 as i32 == HTCAPTION {
                return true;
            }

            // Fallback for modern windows like Windows 11 File Explorer (CabinetWClass)
            // that return HTCLIENT for their custom title bars
            let mut class_name = [0u16; 256];
            let len = GetClassNameW(hwnd, &mut class_name);
            let class = String::from_utf16_lossy(&class_name[..len as usize]);

            if class == "CabinetWClass" {
                let mut rect = windows::Win32::Foundation::RECT::default();
                if GetWindowRect(hwnd, &mut rect).is_ok() {
                    let top_margin = 60; // Approximated title bar height for WinUI 3 tabs
                    if point.y >= rect.top as f64 && point.y <= (rect.top + top_margin) as f64 {
                        return true;
                    }
                }
            }

            false
        }
    }

    fn is_interactive_tab(&self, handle: &WindowHandle, _point: Point) -> bool {
        // 检查是否在 Explorer 标签页上
        // 使用 UI Automation 检测（需要 IUIAutomation 接口）
        unsafe {
            let hwnd = HWND(handle.inner.hwnd as *mut _);

            // 获取窗口类名
            let mut class_name = [0u16; 256];
            let len = GetClassNameW(hwnd, &mut class_name);
            if len == 0 {
                return false;
            }

            let class = String::from_utf16_lossy(&class_name[..len as usize]);

            // 仅对 Explorer 窗口检查标签页
            if class != "CabinetWClass" {
                return false;
            }

            // TODO: 完整的 UI Automation 标签页检测
            // 需要 IUIAutomation::ElementFromPoint 检查 ControlType
            false
        }
    }
}
