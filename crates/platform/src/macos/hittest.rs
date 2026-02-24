use screenhop_core::Point;

use crate::{HitTester, WindowHandle};

/// macOS 命中检测器（基于 AXUIElement role 检测）
pub struct MacHitTester {
    title_bar_height: f64,
}

impl MacHitTester {
    pub fn new() -> Self {
        Self {
            title_bar_height: 40.0,
        }
    }

    pub fn set_title_bar_height(&mut self, height: f64) {
        self.title_bar_height = height;
    }

    /// 获取 AX 元素的 Role 字符串
    fn get_element_role(&self, element: *const std::ffi::c_void) -> Option<String> {
        unsafe {
            extern "C" {
                fn AXUIElementCopyAttributeValue(
                    element: *const std::ffi::c_void,
                    attribute: *const std::ffi::c_void,
                    value: *mut *const std::ffi::c_void,
                ) -> i32;
            }

            use core_foundation::base::TCFType;

            let role_attr = core_foundation::string::CFString::new("AXRole");
            let mut role_ref: *const std::ffi::c_void = std::ptr::null();

            let result = AXUIElementCopyAttributeValue(
                element,
                role_attr.as_concrete_TypeRef() as _,
                &mut role_ref,
            );

            if result == 0 && !role_ref.is_null() {
                let role = core_foundation::string::CFString::wrap_under_create_rule(
                    role_ref as _,
                );
                Some(role.to_string())
            } else {
                None
            }
        }
    }

    /// 获取父元素
    fn get_parent(&self, element: *const std::ffi::c_void) -> Option<*const std::ffi::c_void> {
        unsafe {
            extern "C" {
                fn AXUIElementCopyAttributeValue(
                    element: *const std::ffi::c_void,
                    attribute: *const std::ffi::c_void,
                    value: *mut *const std::ffi::c_void,
                ) -> i32;
            }

            use core_foundation::base::TCFType;

            let parent_attr = core_foundation::string::CFString::new("AXParent");
            let mut parent_ref: *const std::ffi::c_void = std::ptr::null();

            let result = AXUIElementCopyAttributeValue(
                element,
                parent_attr.as_concrete_TypeRef() as _,
                &mut parent_ref,
            );

            if result == 0 && !parent_ref.is_null() {
                Some(parent_ref)
            } else {
                None
            }
        }
    }

    /// 检查元素是否是交互式标签元素（等同 Swift 版的 isInteractiveTabElement）
    fn check_interactive_tab(&self, start_element: *const std::ffi::c_void) -> bool {
        let mut current = start_element;
        let mut depth = 0;
        let max_depth = 10;
        let mut found_interactive = false;

        loop {
            if depth >= max_depth {
                break;
            }

            let role = match self.get_element_role(current) {
                Some(r) => r,
                None => break,
            };

            log::debug!("Depth: {} | Role: {}", depth, role);

            // 在 AXTabGroup 内使用深度判定
            if role == "AXTabGroup" {
                return depth != 1; // depth==1 → TabGroup 空白区, 允许移动
            }

            // 在 AXToolbar 内 → 允许移动
            if role == "AXToolbar" {
                return false;
            }

            // 记录交互元素
            if ["AXStaticText", "AXImage", "AXButton", "AXRadioButton"].contains(&role.as_str()) {
                found_interactive = true;
            }

            // 向上查找
            match self.get_parent(current) {
                Some(parent) => {
                    current = parent;
                    depth += 1;
                }
                None => break,
            }
        }

        found_interactive
    }
}

impl HitTester for MacHitTester {
    fn is_title_bar_hit(&self, handle: &WindowHandle, point: Point) -> bool {
        use crate::WindowManager;
        let wm = super::window::MacWindowManager::new();

        if let Some(frame) = wm.get_window_frame(handle) {
            screenhop_core::monitor::is_in_title_bar(point, &frame, self.title_bar_height)
        } else {
            false
        }
    }

    fn is_interactive_tab(&self, _handle: &WindowHandle, point: Point) -> bool {
        // 获取点击位置的 UI 元素
        let wm = super::window::MacWindowManager::new();
        if let Some(element) = wm.get_element_at_position(point) {
            self.check_interactive_tab(element)
        } else {
            false
        }
    }
}
