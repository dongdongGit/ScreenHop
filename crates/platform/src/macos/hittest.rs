use crate::{HitTester, WindowHandle};
use crate::{Point, Rect};

/// macOS 命中检测器（基于 AXUIElement role 检测）
pub struct MacHitTester {
    title_bar_height: f64,
}

impl MacHitTester {
    const INTERACTIVE_ROLE_MAX_DEPTH: usize = 10;
    const INTERACTIVE_TAB_ROLES: [&'static str; 4] =
        ["AXStaticText", "AXImage", "AXButton", "AXRadioButton"];
    const CHROME_BUNDLE_IDS: [&'static str; 4] = [
        "com.google.Chrome",
        "com.google.Chrome.beta",
        "com.google.Chrome.dev",
        "com.google.Chrome.canary",
    ];
    const CHROME_TAB_STRIP_LEFT_INSET: f64 = 55.0;
    const CHROME_TAB_MIN_WIDTH: f64 = 28.0;
    const CHROME_TAB_MAX_WIDTH: f64 = 360.0;
    const CHROME_TAB_MAX_TITLEBAR_OVERFLOW: f64 = 12.0;

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
        self.get_string_attribute(element, "AXRole")
    }

    fn get_string_attribute(
        &self,
        element: *const std::ffi::c_void,
        attribute_name: &str,
    ) -> Option<String> {
        unsafe {
            extern "C" {
                fn AXUIElementCopyAttributeValue(
                    element: *const std::ffi::c_void,
                    attribute: *const std::ffi::c_void,
                    value: *mut *const std::ffi::c_void,
                ) -> i32;
                fn CFGetTypeID(cf: *const std::ffi::c_void) -> usize;
                fn CFRelease(cf: *const std::ffi::c_void);
                fn CFStringGetTypeID() -> usize;
            }

            use core_foundation::base::TCFType;

            let attr = core_foundation::string::CFString::new(attribute_name);
            let mut value_ref: *const std::ffi::c_void = std::ptr::null();

            let result = AXUIElementCopyAttributeValue(
                element,
                attr.as_concrete_TypeRef() as _,
                &mut value_ref,
            );

            if result != 0 || value_ref.is_null() {
                return None;
            }

            if CFGetTypeID(value_ref) != CFStringGetTypeID() {
                CFRelease(value_ref);
                return None;
            }

            let value = core_foundation::string::CFString::wrap_under_create_rule(value_ref as _);
            let text = Self::sanitize_ax_text(&value.to_string());

            if text.is_empty() {
                None
            } else {
                Some(text)
            }
        }
    }

    fn sanitize_ax_text(text: &str) -> String {
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        let mut chars = text.chars();
        let truncated = chars.by_ref().take(80).collect::<String>();

        if chars.next().is_some() {
            format!("{}...", truncated)
        } else {
            truncated
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

    fn get_element_frame(&self, element: *const std::ffi::c_void) -> Option<Rect> {
        unsafe {
            extern "C" {
                fn AXUIElementCopyAttributeValue(
                    element: *const std::ffi::c_void,
                    attribute: *const std::ffi::c_void,
                    value: *mut *const std::ffi::c_void,
                ) -> i32;
                fn AXValueGetValue(
                    value: *const std::ffi::c_void,
                    value_type: u32,
                    value_ptr: *mut std::ffi::c_void,
                ) -> bool;
                fn CFRelease(cf: *const std::ffi::c_void);
            }

            use core_foundation::base::TCFType;

            let pos_attr = core_foundation::string::CFString::new("AXPosition");
            let size_attr = core_foundation::string::CFString::new("AXSize");

            let mut pos_ref: *const std::ffi::c_void = std::ptr::null();
            let result = AXUIElementCopyAttributeValue(
                element,
                pos_attr.as_concrete_TypeRef() as _,
                &mut pos_ref,
            );
            if result != 0 || pos_ref.is_null() {
                return None;
            }

            let mut size_ref: *const std::ffi::c_void = std::ptr::null();
            let result = AXUIElementCopyAttributeValue(
                element,
                size_attr.as_concrete_TypeRef() as _,
                &mut size_ref,
            );
            if result != 0 || size_ref.is_null() {
                CFRelease(pos_ref);
                return None;
            }

            let mut point = core_graphics::geometry::CGPoint::new(0.0, 0.0);
            let mut size = core_graphics::geometry::CGSize::new(0.0, 0.0);

            if !AXValueGetValue(pos_ref, 1, &mut point as *mut _ as *mut _) {
                CFRelease(pos_ref);
                CFRelease(size_ref);
                return None;
            }
            if !AXValueGetValue(size_ref, 2, &mut size as *mut _ as *mut _) {
                CFRelease(pos_ref);
                CFRelease(size_ref);
                return None;
            }

            CFRelease(pos_ref);
            CFRelease(size_ref);

            Some(Rect::new(point.x, point.y, size.width, size.height))
        }
    }

    fn is_in_title_bar(point: Point, window_frame: &Rect, title_bar_height: f64) -> bool {
        point.x >= window_frame.min_x()
            && point.x <= window_frame.max_x()
            && point.y >= window_frame.min_y()
            && point.y <= window_frame.min_y() + title_bar_height
    }

    fn format_ax_hierarchy_line(
        depth: usize,
        role: Option<&str>,
        subrole: Option<&str>,
        title: Option<&str>,
        identifier: Option<&str>,
        description: Option<&str>,
        frame: Option<&Rect>,
        contains_point: bool,
    ) -> String {
        let mut parts = vec![format!("Depth: {}", depth)];

        if let Some(role) = role {
            parts.push(format!("Role: {}", role));
        }
        if let Some(subrole) = subrole {
            parts.push(format!("Subrole: {}", subrole));
        }
        if let Some(title) = title {
            parts.push(format!("Title: {}", title));
        }
        if let Some(identifier) = identifier {
            parts.push(format!("Identifier: {}", identifier));
        }
        if let Some(description) = description {
            parts.push(format!("Description: {}", description));
        }
        if let Some(frame) = frame {
            parts.push(format!(
                "Frame: ({:.0},{:.0},{:.0},{:.0})",
                frame.x, frame.y, frame.width, frame.height
            ));
        }

        parts.push(format!("ContainsPoint: {}", contains_point));
        parts.join(" | ")
    }

    fn log_ax_hierarchy(&self, label: &str, start_element: *const std::ffi::c_void, point: Point) {
        log::debug!(
            "AX 层级 [{}] 点击点: ({:.0},{:.0})",
            label,
            point.x,
            point.y
        );

        let mut current = start_element;

        for depth in 0..Self::INTERACTIVE_ROLE_MAX_DEPTH {
            let role = self.get_element_role(current);
            let subrole = self.get_string_attribute(current, "AXSubrole");
            let title = self.get_string_attribute(current, "AXTitle");
            let identifier = self.get_string_attribute(current, "AXIdentifier");
            let description = self.get_string_attribute(current, "AXDescription");
            let frame = self.get_element_frame(current);
            let contains_point = frame.map(|f| f.contains(point)).unwrap_or(false);

            log::debug!(
                "{}",
                Self::format_ax_hierarchy_line(
                    depth,
                    role.as_deref(),
                    subrole.as_deref(),
                    title.as_deref(),
                    identifier.as_deref(),
                    description.as_deref(),
                    frame.as_ref(),
                    contains_point,
                )
            );

            if role.as_deref() == Some("AXApplication") {
                break;
            }

            match self.get_parent(current) {
                Some(parent) => current = parent,
                None => break,
            }
        }
    }

    /// 检查元素是否是交互式标签元素（等同 Swift 版的 isInteractiveTabElement）
    fn check_interactive_tab(&self, start_element: *const std::ffi::c_void) -> bool {
        let mut current = start_element;
        let mut depth = 0;
        let mut role_chain = Vec::new();

        loop {
            if depth >= Self::INTERACTIVE_ROLE_MAX_DEPTH {
                break;
            }

            let role = match self.get_element_role(current) {
                Some(r) => r,
                None => break,
            };

            log::debug!("Depth: {} | Role: {}", depth, role);

            role_chain.push(role);

            // 向上查找
            match self.get_parent(current) {
                Some(parent) => {
                    current = parent;
                    depth += 1;
                }
                None => break,
            }
        }

        Self::is_interactive_tab_role_chain(role_chain.iter().map(String::as_str))
    }

    fn is_interactive_tab_role_chain<I, S>(roles: I) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut found_interactive = false;

        for (depth, role) in roles
            .into_iter()
            .take(Self::INTERACTIVE_ROLE_MAX_DEPTH)
            .enumerate()
        {
            let role = role.as_ref();

            if role == "AXTabGroup" {
                return found_interactive || depth != 1;
            }

            if role == "AXToolbar" {
                return false;
            }

            if Self::INTERACTIVE_TAB_ROLES.contains(&role) {
                found_interactive = true;
            }
        }

        found_interactive
    }

    fn is_chrome_bundle_id(bundle_id: Option<&str>) -> bool {
        bundle_id
            .map(|id| Self::CHROME_BUNDLE_IDS.contains(&id))
            .unwrap_or(false)
    }

    fn is_chrome_title_bar_interactive_decision(
        chrome_tab_hit: bool,
        _generic_role_chain_hit: bool,
    ) -> bool {
        chrome_tab_hit
    }

    fn check_chrome_tab_element(
        &self,
        start_element: *const std::ffi::c_void,
        point: Point,
        window_frame: &Rect,
        title_bar_height: f64,
    ) -> bool {
        let mut current = start_element;

        for depth in 0..Self::INTERACTIVE_ROLE_MAX_DEPTH {
            let role = self.get_element_role(current);
            let element_frame = self.get_element_frame(current);

            if let (Some(role), Some(element_frame)) = (role.as_deref(), element_frame) {
                log::debug!(
                    "Chrome Depth: {} | Role: {} | Frame: ({:.0},{:.0},{:.0},{:.0})",
                    depth,
                    role,
                    element_frame.x,
                    element_frame.y,
                    element_frame.width,
                    element_frame.height
                );

                if Self::is_chrome_tab_candidate_role(role)
                    && Self::is_chrome_tab_like_frame(
                        point,
                        window_frame,
                        &element_frame,
                        title_bar_height,
                    )
                {
                    return true;
                }

                if role == "AXWindow" || role == "AXApplication" {
                    break;
                }
            }

            if self.check_chrome_descendant_tab_element(
                current,
                point,
                window_frame,
                title_bar_height,
                depth + 1,
            ) {
                return true;
            }

            match self.get_parent(current) {
                Some(parent) => current = parent,
                None => break,
            }
        }

        false
    }

    fn check_chrome_descendant_tab_element(
        &self,
        element: *const std::ffi::c_void,
        point: Point,
        window_frame: &Rect,
        title_bar_height: f64,
        depth: usize,
    ) -> bool {
        if depth >= Self::INTERACTIVE_ROLE_MAX_DEPTH {
            return false;
        }

        unsafe {
            extern "C" {
                fn AXUIElementCopyAttributeValue(
                    element: *const std::ffi::c_void,
                    attribute: *const std::ffi::c_void,
                    value: *mut *const std::ffi::c_void,
                ) -> i32;
                fn CFArrayGetCount(the_array: *const std::ffi::c_void) -> isize;
                fn CFArrayGetValueAtIndex(
                    the_array: *const std::ffi::c_void,
                    idx: isize,
                ) -> *const std::ffi::c_void;
                fn CFRelease(cf: *const std::ffi::c_void);
            }

            use core_foundation::base::TCFType;

            let children_attr = core_foundation::string::CFString::new("AXChildren");
            let mut children_ref: *const std::ffi::c_void = std::ptr::null();

            let result = AXUIElementCopyAttributeValue(
                element,
                children_attr.as_concrete_TypeRef() as _,
                &mut children_ref,
            );
            if result != 0 || children_ref.is_null() {
                return false;
            }

            let mut found = false;
            let count = CFArrayGetCount(children_ref);

            for index in 0..count {
                let child = CFArrayGetValueAtIndex(children_ref, index);
                if child.is_null() {
                    continue;
                }

                if self.is_chrome_tab_element_match(
                    child,
                    point,
                    window_frame,
                    title_bar_height,
                    depth,
                ) || self.check_chrome_descendant_tab_element(
                    child,
                    point,
                    window_frame,
                    title_bar_height,
                    depth + 1,
                ) {
                    found = true;
                    break;
                }
            }

            CFRelease(children_ref);
            found
        }
    }

    fn is_chrome_tab_element_match(
        &self,
        element: *const std::ffi::c_void,
        point: Point,
        window_frame: &Rect,
        title_bar_height: f64,
        depth: usize,
    ) -> bool {
        let role = self.get_element_role(element);
        let element_frame = self.get_element_frame(element);

        if let (Some(role), Some(element_frame)) = (role.as_deref(), element_frame) {
            log::debug!(
                "Chrome Child Depth: {} | Role: {} | Frame: ({:.0},{:.0},{:.0},{:.0})",
                depth,
                role,
                element_frame.x,
                element_frame.y,
                element_frame.width,
                element_frame.height
            );

            Self::is_chrome_tab_candidate_role(role)
                && Self::is_chrome_tab_like_frame(
                    point,
                    window_frame,
                    &element_frame,
                    title_bar_height,
                )
        } else {
            false
        }
    }

    fn is_chrome_tab_candidate_role(role: &str) -> bool {
        matches!(
            role,
            "AXStaticText" | "AXImage" | "AXButton" | "AXRadioButton"
        )
    }

    fn is_chrome_tab_like_frame(
        point: Point,
        window_frame: &Rect,
        element_frame: &Rect,
        title_bar_height: f64,
    ) -> bool {
        if !Self::is_in_title_bar(point, window_frame, title_bar_height)
            || !element_frame.contains(point)
        {
            return false;
        }

        let title_bar_min_y = window_frame.min_y();
        let title_bar_max_y = window_frame.min_y() + title_bar_height;
        let max_tab_width = Self::CHROME_TAB_MAX_WIDTH.min(window_frame.width * 0.5);

        element_frame.min_x() >= window_frame.min_x() + Self::CHROME_TAB_STRIP_LEFT_INSET
            && element_frame.min_y() >= title_bar_min_y - Self::CHROME_TAB_MAX_TITLEBAR_OVERFLOW
            && element_frame.max_y() <= title_bar_max_y + Self::CHROME_TAB_MAX_TITLEBAR_OVERFLOW
            && element_frame.width >= Self::CHROME_TAB_MIN_WIDTH
            && element_frame.width <= max_tab_width
            && element_frame.height <= title_bar_height + Self::CHROME_TAB_MAX_TITLEBAR_OVERFLOW
    }

    fn get_bundle_identifier(pid: i32) -> Option<String> {
        unsafe {
            use objc::runtime::Object;
            use objc::*;
            use std::ffi::CStr;
            use std::os::raw::c_char;

            let app: *mut Object = msg_send![
                class!(NSRunningApplication),
                runningApplicationWithProcessIdentifier: pid
            ];
            if app.is_null() {
                return None;
            }

            let bundle_id: *mut Object = msg_send![app, bundleIdentifier];
            if bundle_id.is_null() {
                return None;
            }

            let utf8: *const c_char = msg_send![bundle_id, UTF8String];
            if utf8.is_null() {
                return None;
            }

            Some(CStr::from_ptr(utf8).to_string_lossy().into_owned())
        }
    }
}

impl HitTester for MacHitTester {
    fn is_title_bar_hit(&self, handle: &WindowHandle, point: Point) -> bool {
        use crate::WindowManager;
        let wm = super::window::MacWindowManager::new();

        if let Some(frame) = wm.get_window_frame(handle) {
            Self::is_in_title_bar(point, &frame, self.title_bar_height)
        } else {
            false
        }
    }

    fn is_interactive_tab(&self, handle: &WindowHandle, point: Point) -> bool {
        use crate::WindowManager;

        let wm = super::window::MacWindowManager::new();
        let element = wm.get_element_at_position(point);

        if let Some(frame) = wm.get_window_frame(handle) {
            let bundle_id = Self::get_bundle_identifier(handle.inner.pid);
            let in_title_bar = Self::is_in_title_bar(point, &frame, self.title_bar_height);

            log::debug!(
                "HitTest 点击: point=({:.0},{:.0}) window=({:.0},{:.0},{:.0},{:.0}) title_bar_height={:.0} in_title_bar={} bundle_id={}",
                point.x,
                point.y,
                frame.x,
                frame.y,
                frame.width,
                frame.height,
                self.title_bar_height,
                in_title_bar,
                bundle_id.as_deref().unwrap_or("<unknown>")
            );

            if let Some(element) = element {
                if log::log_enabled!(log::Level::Debug) {
                    self.log_ax_hierarchy("clicked element -> parents", element, point);
                }
            } else {
                log::debug!("AX 层级 [clicked element -> parents] 未获取到 AX 元素");
            }

            if Self::is_chrome_bundle_id(bundle_id.as_deref()) && in_title_bar {
                if let Some(element) = element {
                    let chrome_tab_hit = self.check_chrome_tab_element(
                        element,
                        point,
                        &frame,
                        self.title_bar_height,
                    );

                    log::debug!("Chrome 判定: chrome_tab_hit={}", chrome_tab_hit);

                    if Self::is_chrome_title_bar_interactive_decision(chrome_tab_hit, false) {
                        log::debug!("Chrome 标签页点击，跳过窗口移动");
                        return true;
                    }

                    log::debug!("Chrome 标题栏空白点击，允许窗口移动");
                    return false;
                }

                log::debug!("Chrome 标题栏未获取到 AX 元素，允许窗口移动");
                return false;
            }
        } else if let Some(element) = element {
            log::debug!(
                "HitTest 点击: point=({:.0},{:.0}) 未获取到窗口 frame",
                point.x,
                point.y
            );
            self.log_ax_hierarchy("clicked element -> parents", element, point);
        } else {
            log::debug!(
                "HitTest 点击: point=({:.0},{:.0}) 未获取到窗口 frame 或 AX 元素",
                point.x,
                point.y
            );
        }

        if let Some(element) = element {
            self.check_interactive_tab(element)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::MacHitTester;
    use crate::{Point, Rect};

    #[test]
    fn chrome_tab_role_chain_without_tab_group_is_interactive() {
        let roles = ["AXStaticText", "AXGroup", "AXWindow"];

        assert!(MacHitTester::is_interactive_tab_role_chain(
            roles.iter().copied()
        ));
    }

    #[test]
    fn chrome_tab_button_inside_tab_group_is_interactive() {
        let roles = ["AXRadioButton", "AXTabGroup", "AXWindow"];

        assert!(MacHitTester::is_interactive_tab_role_chain(
            roles.iter().copied()
        ));
    }

    #[test]
    fn tab_group_blank_area_allows_move() {
        let roles = ["AXGroup", "AXTabGroup", "AXWindow"];

        assert!(!MacHitTester::is_interactive_tab_role_chain(
            roles.iter().copied()
        ));
    }

    #[test]
    fn toolbar_control_allows_move() {
        let roles = ["AXButton", "AXToolbar", "AXWindow"];

        assert!(!MacHitTester::is_interactive_tab_role_chain(
            roles.iter().copied()
        ));
    }

    #[test]
    fn chrome_title_bar_ignores_generic_role_chain_when_frame_misses() {
        assert!(!MacHitTester::is_chrome_title_bar_interactive_decision(
            false, true
        ));
    }

    #[test]
    fn chrome_title_bar_uses_chrome_frame_hit() {
        assert!(MacHitTester::is_chrome_title_bar_interactive_decision(
            true, false
        ));
    }

    #[test]
    fn chrome_plain_group_is_not_tab_candidate() {
        assert!(!MacHitTester::is_chrome_tab_candidate_role("AXGroup"));
    }

    #[test]
    fn formats_ax_hierarchy_line_with_component_details() {
        let frame = Rect::new(10.0, 20.0, 300.0, 40.0);

        assert_eq!(
            MacHitTester::format_ax_hierarchy_line(
                2,
                Some("AXGroup"),
                Some("AXTabGroup"),
                Some("新标签"),
                Some("tab-1"),
                Some("标签描述"),
                Some(&frame),
                true,
            ),
            "Depth: 2 | Role: AXGroup | Subrole: AXTabGroup | Title: 新标签 | Identifier: tab-1 | Description: 标签描述 | Frame: (10,20,300,40) | ContainsPoint: true"
        );
    }

    #[test]
    fn chrome_tab_radio_button_frame_is_interactive() {
        let window_frame = Rect::new(0.0, 25.0, 1200.0, 800.0);
        let element_frame = Rect::new(80.0, 28.0, 220.0, 32.0);
        let point = Point { x: 180.0, y: 40.0 };

        assert!(MacHitTester::is_chrome_tab_like_frame(
            point,
            &window_frame,
            &element_frame,
            40.0
        ));
    }

    #[test]
    fn chrome_tab_radio_button_frame_past_old_percentage_is_interactive() {
        let window_frame = Rect::new(0.0, 25.0, 1200.0, 800.0);
        let element_frame = Rect::new(960.0, 28.0, 115.0, 32.0);
        let point = Point { x: 1015.0, y: 40.0 };

        assert!(MacHitTester::is_chrome_tab_like_frame(
            point,
            &window_frame,
            &element_frame,
            40.0
        ));
    }

    #[test]
    fn chrome_wide_tab_strip_container_frame_allows_move() {
        let window_frame = Rect::new(0.0, 25.0, 1200.0, 800.0);
        let element_frame = Rect::new(70.0, 25.0, 900.0, 40.0);
        let point = Point { x: 620.0, y: 40.0 };

        assert!(!MacHitTester::is_chrome_tab_like_frame(
            point,
            &window_frame,
            &element_frame,
            40.0
        ));
    }

    #[test]
    fn chrome_window_control_frame_allows_move() {
        let window_frame = Rect::new(0.0, 25.0, 1200.0, 800.0);
        let element_frame = Rect::new(18.0, 34.0, 18.0, 18.0);
        let point = Point { x: 27.0, y: 43.0 };

        assert!(!MacHitTester::is_chrome_tab_like_frame(
            point,
            &window_frame,
            &element_frame,
            40.0
        ));
    }

    #[test]
    fn chrome_content_area_frame_allows_move() {
        let window_frame = Rect::new(0.0, 25.0, 1200.0, 800.0);
        let element_frame = Rect::new(80.0, 90.0, 220.0, 32.0);
        let point = Point { x: 300.0, y: 120.0 };

        assert!(!MacHitTester::is_chrome_tab_like_frame(
            point,
            &window_frame,
            &element_frame,
            40.0
        ));
    }

    #[test]
    fn chrome_title_bar_blank_area_frame_allows_move() {
        let window_frame = Rect::new(0.0, 25.0, 1200.0, 800.0);
        let element_frame = Rect::new(70.0, 25.0, 1100.0, 40.0);
        let point = Point { x: 1000.0, y: 40.0 };

        assert!(!MacHitTester::is_chrome_tab_like_frame(
            point,
            &window_frame,
            &element_frame,
            40.0
        ));
    }

    #[test]
    fn chrome_title_bar_blank_area_inside_old_fallback_region_allows_move() {
        let window_frame = Rect::new(0.0, 25.0, 1200.0, 800.0);
        let element_frame = Rect::new(70.0, 25.0, 900.0, 40.0);
        let point = Point { x: 620.0, y: 40.0 };

        assert!(!MacHitTester::is_chrome_tab_like_frame(
            point,
            &window_frame,
            &element_frame,
            40.0
        ));
    }
}
