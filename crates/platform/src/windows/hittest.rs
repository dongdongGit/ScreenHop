use screenhop_core::Point;

use crate::{HitTester, WindowHandle};

use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_NCHITTEST: u32 = 0x0084;
const HTCAPTION: i32 = 2;

/// Windows 命中检测器（基于 WM_NCHITTEST + UI Automation）
pub struct WinHitTester;

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
            let lparam = ((point.y as i32 & 0xFFFF) << 16) | (point.x as i32 & 0xFFFF);
            let result = SendMessageW(
                hwnd,
                WM_NCHITTEST,
                WPARAM(0),
                LPARAM(lparam as isize),
            );

            result.0 as i32 == HTCAPTION
        }
    }

    fn is_interactive_tab(&self, handle: &WindowHandle, point: Point) -> bool {
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
