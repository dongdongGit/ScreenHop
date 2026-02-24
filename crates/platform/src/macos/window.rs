#![allow(deprecated)]

use anyhow::Result;
use screenhop_core::{Point, Rect};

use crate::{WindowHandle, WindowManager};
use super::MacWindowHandle;

/// macOS 窗口管理器（基于 AXUIElement API）
pub struct MacWindowManager;

impl MacWindowManager {
    pub fn new() -> Self {
        Self
    }

    /// 通过 AXUIElement API 获取指定位置的 UI 元素
    pub(crate) fn get_element_at_position(&self, point: Point) -> Option<*const std::ffi::c_void> {
        unsafe {
            extern "C" {
                fn AXUIElementCreateSystemWide() -> *const std::ffi::c_void;
                fn AXUIElementCopyElementAtPosition(
                    application: *const std::ffi::c_void,
                    x: f32,
                    y: f32,
                    element: *mut *const std::ffi::c_void,
                ) -> i32;
                fn CFRelease(cf: *const std::ffi::c_void);
            }

            let system_wide = AXUIElementCreateSystemWide();
            let mut element: *const std::ffi::c_void = std::ptr::null();

            let result = AXUIElementCopyElementAtPosition(
                system_wide,
                point.x as f32,
                point.y as f32,
                &mut element,
            );

            CFRelease(system_wide);

            if result == 0 && !element.is_null() {
                Some(element)
            } else {
                None
            }
        }
    }

    /// 从 UI 元素向上查找窗口
    fn find_window_from_element(&self, element: *const std::ffi::c_void) -> Option<MacWindowHandle> {
        unsafe {
            extern "C" {
                fn AXUIElementCopyAttributeValue(
                    element: *const std::ffi::c_void,
                    attribute: *const std::ffi::c_void,
                    value: *mut *const std::ffi::c_void,
                ) -> i32;
                fn AXUIElementGetPid(
                    element: *const std::ffi::c_void,
                    pid: *mut i32,
                ) -> i32;
            }

            use core_foundation::base::TCFType;

            // 尝试获取 kAXWindowAttribute
            let window_attr = core_foundation::string::CFString::new("AXWindow");
            let mut window_ref: *const std::ffi::c_void = std::ptr::null();

            let result = AXUIElementCopyAttributeValue(
                element,
                window_attr.as_concrete_TypeRef() as _,
                &mut window_ref,
            );

            let window_element = if result == 0 && !window_ref.is_null() {
                window_ref
            } else {
                // 回退：向上遍历找 AXWindow role
                self.walk_up_to_window(element)?
            };

            let mut pid: i32 = 0;
            AXUIElementGetPid(window_element, &mut pid);

            Some(MacWindowHandle {
                ax_element: window_element,
                pid,
            })
        }
    }

    /// 向上遍历 UI 层级找到窗口
    fn walk_up_to_window(&self, start: *const std::ffi::c_void) -> Option<*const std::ffi::c_void> {
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
            let parent_attr = core_foundation::string::CFString::new("AXParent");
            let window_role = "AXWindow";

            let mut current = start;
            for _ in 0..20 {
                // 检查 role
                let mut role_ref: *const std::ffi::c_void = std::ptr::null();
                let r = AXUIElementCopyAttributeValue(
                    current,
                    role_attr.as_concrete_TypeRef() as _,
                    &mut role_ref,
                );
                if r == 0 && !role_ref.is_null() {
                    let role_cfstr = core_foundation::string::CFString::wrap_under_create_rule(
                        role_ref as _,
                    );
                    if role_cfstr.to_string() == window_role {
                        return Some(current);
                    }
                }

                // 向上获取 parent
                let mut parent_ref: *const std::ffi::c_void = std::ptr::null();
                let r = AXUIElementCopyAttributeValue(
                    current,
                    parent_attr.as_concrete_TypeRef() as _,
                    &mut parent_ref,
                );
                if r != 0 || parent_ref.is_null() {
                    break;
                }
                current = parent_ref;
            }

            None
        }
    }
}

impl WindowManager for MacWindowManager {
    fn get_window_at(&self, point: Point) -> Option<WindowHandle> {
        let element = self.get_element_at_position(point)?;
        let mac_handle = self.find_window_from_element(element)?;
        Some(WindowHandle { inner: mac_handle })
    }

    fn get_window_frame(&self, handle: &WindowHandle) -> Option<Rect> {
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
            }

            use core_foundation::base::TCFType;

            let pos_attr = core_foundation::string::CFString::new("AXPosition");
            let size_attr = core_foundation::string::CFString::new("AXSize");

            let element = handle.inner.ax_element;

            // 获取位置
            let mut pos_ref: *const std::ffi::c_void = std::ptr::null();
            let r = AXUIElementCopyAttributeValue(
                element,
                pos_attr.as_concrete_TypeRef() as _,
                &mut pos_ref,
            );
            if r != 0 || pos_ref.is_null() {
                return None;
            }

            // 获取尺寸
            let mut size_ref: *const std::ffi::c_void = std::ptr::null();
            let r = AXUIElementCopyAttributeValue(
                element,
                size_attr.as_concrete_TypeRef() as _,
                &mut size_ref,
            );
            if r != 0 || size_ref.is_null() {
                return None;
            }

            // AXValueType: kAXValueCGPointType = 1, kAXValueCGSizeType = 2
            let mut point = core_graphics::geometry::CGPoint::new(0.0, 0.0);
            let mut size = core_graphics::geometry::CGSize::new(0.0, 0.0);

            AXValueGetValue(pos_ref, 1, &mut point as *mut _ as *mut _);
            AXValueGetValue(size_ref, 2, &mut size as *mut _ as *mut _);

            Some(Rect::new(point.x, point.y, size.width, size.height))
        }
    }

    fn set_window_position(&self, handle: &WindowHandle, pos: Point) -> Result<()> {
        unsafe {
            extern "C" {
                fn AXUIElementSetAttributeValue(
                    element: *const std::ffi::c_void,
                    attribute: *const std::ffi::c_void,
                    value: *const std::ffi::c_void,
                ) -> i32;
                fn AXValueCreate(
                    value_type: u32,
                    value_ptr: *const std::ffi::c_void,
                ) -> *const std::ffi::c_void;
            }

            use core_foundation::base::TCFType;

            let pos_attr = core_foundation::string::CFString::new("AXPosition");
            let mut cg_point = core_graphics::geometry::CGPoint::new(pos.x, pos.y);

            let value = AXValueCreate(1, &mut cg_point as *mut _ as *const _);
            if !value.is_null() {
                AXUIElementSetAttributeValue(
                    handle.inner.ax_element,
                    pos_attr.as_concrete_TypeRef() as _,
                    value,
                );
            }
        }
        Ok(())
    }

    fn set_window_size(&self, handle: &WindowHandle, width: f64, height: f64) -> Result<()> {
        unsafe {
            extern "C" {
                fn AXUIElementSetAttributeValue(
                    element: *const std::ffi::c_void,
                    attribute: *const std::ffi::c_void,
                    value: *const std::ffi::c_void,
                ) -> i32;
                fn AXValueCreate(
                    value_type: u32,
                    value_ptr: *const std::ffi::c_void,
                ) -> *const std::ffi::c_void;
            }

            use core_foundation::base::TCFType;

            let size_attr = core_foundation::string::CFString::new("AXSize");
            let mut cg_size = core_graphics::geometry::CGSize::new(width, height);

            let value = AXValueCreate(2, &mut cg_size as *mut _ as *const _);
            if !value.is_null() {
                AXUIElementSetAttributeValue(
                    handle.inner.ax_element,
                    size_attr.as_concrete_TypeRef() as _,
                    value,
                );
            }
        }
        Ok(())
    }

    fn activate_window(&self, handle: &WindowHandle) -> Result<()> {
        unsafe {
            extern "C" {
                fn AXUIElementSetAttributeValue(
                    element: *const std::ffi::c_void,
                    attribute: *const std::ffi::c_void,
                    value: *const std::ffi::c_void,
                ) -> i32;
                fn AXUIElementPerformAction(
                    element: *const std::ffi::c_void,
                    action: *const std::ffi::c_void,
                ) -> i32;
            }

            use core_foundation::base::TCFType;

            let element = handle.inner.ax_element;

            // 设置 AXMain = true
            let main_attr = core_foundation::string::CFString::new("AXMain");
            let true_val = core_foundation::boolean::CFBoolean::true_value();
            AXUIElementSetAttributeValue(
                element,
                main_attr.as_concrete_TypeRef() as _,
                true_val.as_concrete_TypeRef() as _,
            );

            // 执行 AXRaise
            let raise_action = core_foundation::string::CFString::new("AXRaise");
            AXUIElementPerformAction(element, raise_action.as_concrete_TypeRef() as _);

            // 激活应用
            let pid = handle.inner.pid;
            use objc::runtime::Object;
            use objc::*;

            let cls = class!(NSRunningApplication);
            let app: *mut Object = msg_send![
                cls,
                runningApplicationWithProcessIdentifier: pid
            ];
            if !app.is_null() {
                let _: () = msg_send![
                    app,
                    activateWithOptions: 0x01u64 // NSApplicationActivateAllWindows
                ];
            }
        }
        Ok(())
    }

    fn is_maximized(&self, _handle: &WindowHandle) -> bool {
        // macOS 没有传统意义上的 "最大化" 状态
        // 可以通过比较窗口 frame 和屏幕 frame 来近似判断
        false
    }

    fn restore_window(&self, _handle: &WindowHandle) -> Result<()> {
        // macOS 无需特殊还原操作
        Ok(())
    }

    fn maximize_window(&self, _handle: &WindowHandle) -> Result<()> {
        // macOS 无需特殊最大化操作
        Ok(())
    }
}
