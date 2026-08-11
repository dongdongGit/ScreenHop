/// 跨平台的弹窗宏，简化 Windows MessageBox / macOS osascript 的调用
///
/// 用法:
/// ```no_run
/// use screenhop::alert;
/// alert!("标题", "内容文本");
/// ```
#[macro_export]
macro_rules! alert {
    ($title:expr, $msg:expr) => {{
        #[cfg(target_os = "windows")]
        {
            use windows::core::{HSTRING, PCWSTR};
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
            let h_text = HSTRING::from($msg);
            let h_title = HSTRING::from($title);
            unsafe {
                MessageBoxW(
                    None,
                    PCWSTR(h_text.as_ptr()),
                    PCWSTR(h_title.as_ptr()),
                    MB_ICONINFORMATION | MB_OK,
                );
            }
        }
        #[cfg(target_os = "macos")]
        {
            let script = format!(
                "display dialog \"{}\" with title \"{}\" buttons {{\"确定\"}} default button 1",
                $msg, $title
            );
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output();
        }
    }};
}

/// 跨平台的确认弹窗宏，返回 bool
///
/// 用法:
/// ```no_run
/// use screenhop::confirm;
/// let confirmed = confirm!("标题", "是否继续？");
/// ```
#[macro_export]
macro_rules! confirm {
    ($title:expr, $msg:expr) => {{
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OKCANCEL, IDOK};
            use windows::core::{HSTRING, PCWSTR};
            let h_text = HSTRING::from($msg);
            let h_title = HSTRING::from($title);
            unsafe {
                MessageBoxW(
                    None,
                    PCWSTR(h_text.as_ptr()),
                    PCWSTR(h_title.as_ptr()),
                    MB_ICONINFORMATION | MB_OKCANCEL,
                ) == IDOK
            }
        }
        #[cfg(target_os = "macos")]
        {
            let mut flag = false;
            let script = format!(
                "display dialog \"{}\" with title \"{}\" buttons {{\"取消\", \"确定\"}} default button 2",
                $msg, $title
            );
            if let Ok(out) = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output()
            {
                if String::from_utf8_lossy(&out.stdout).contains("确定") {
                    flag = true;
                }
            }
            flag
        }
    }};
}

/// 跨平台的错误弹窗宏
#[macro_export]
macro_rules! alert_error {
    ($title:expr, $msg:expr) => {{
        #[cfg(target_os = "windows")]
        {
            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};
            use windows::core::{HSTRING, PCWSTR};
            let h_text = HSTRING::from($msg);
            let h_title = HSTRING::from($title);
            unsafe {
                MessageBoxW(
                    None,
                    PCWSTR(h_text.as_ptr()),
                    PCWSTR(h_title.as_ptr()),
                    MB_ICONERROR | MB_OK,
                );
            }
        }
        #[cfg(target_os = "macos")]
        {
            let script = format!(
                "display dialog \"{}\" with title \"{}\" buttons {{\"确定\"}} default button 1 with icon stop",
                $msg, $title
            );
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(&script)
                .output();
        }
    }};
}
