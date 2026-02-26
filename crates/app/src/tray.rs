use anyhow::Result;
use slint::ComponentHandle;
use muda::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{menu::MenuEvent, Icon, TrayIconBuilder};
use screenhop_core::config::AppConfig;

use std::sync::{Arc, Mutex};

const MENU_ID_TOGGLE: &str = "toggle";
const MENU_ID_AUTOSTART: &str = "autostart";
const MENU_ID_CHECK_UPDATE: &str = "check_update";
const MENU_ID_PROXY_ENABLE: &str = "proxy_enable";
const MENU_ID_PROXY_SETTINGS: &str = "proxy_settings";
const MENU_ID_AUTO_CHECK_UPDATE: &str = "auto_check_update";
const MENU_ID_QUIT: &str = "quit";

/// 创建托盘图标（使用真实的 png）
fn create_tray_icon_image() -> Icon {
    #[cfg(target_os = "macos")]
    let icon_data = include_bytes!("../../../assets/icon-mac-tray.png");
    #[cfg(not(target_os = "macos"))]
    let icon_data = include_bytes!("../../../assets/icon-32.png");

    let image = image::load_from_memory(icon_data)
        .expect("无法加载托盘图标数据")
        .into_rgba8();
    let (width, height) = image.dimensions();
    let rgba = image.into_raw();
    Icon::from_rgba(rgba, width, height).expect("创建托盘图标失败")
}

use std::sync::atomic::{AtomicBool, Ordering};

/// 在后台线程中执行更新检查和下载
fn do_check_update(
    version: &str,
    proxy_url: Option<String>,
    proxy_username: Option<String>,
    proxy_password: Option<String>,
    manual: bool,
) {
    let version_str = version.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            match screenhop_core::updater::check_for_update(&version_str, proxy_url.as_deref(), proxy_username.as_deref(), proxy_password.as_deref()).await {
                Ok(result) => {
                    if result.has_update {
                        log::info!(
                            "发现新版本: {} → {}",
                            result.current_version,
                            result.latest_version
                        );
                        
                        #[cfg(target_os = "macos")]
                        let should_update = {
                            let mut flag = false;
                            let script = format!(
                                "display dialog \"发现新版本: {} \\n是否立即下载并安装？\" with title \"ScreenHop 更新\" buttons {{\"稍后\", \"立即更新\"}} default button 2",
                                result.latest_version
                            );
                            if let Ok(out) = std::process::Command::new("osascript").arg("-e").arg(&script).output() {
                                if String::from_utf8_lossy(&out.stdout).contains("立即更新") {
                                    flag = true;
                                }
                            }
                            flag
                        };
                        
                        #[cfg(target_os = "windows")]
                        let should_update = {
                            use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OKCANCEL, IDOK};
                            use windows::core::{HSTRING, PCWSTR};
                            
                            let text = format!("发现新版本: {}\n是否立即下载并安装？", result.latest_version);
                            let title = "ScreenHop 更新";
                            
                            let h_text = HSTRING::from(text);
                            let h_title = HSTRING::from(title);
                            
                            unsafe {
                                MessageBoxW(
                                    None,
                                    PCWSTR(h_text.as_ptr()),
                                    PCWSTR(h_title.as_ptr()),
                                    MB_ICONINFORMATION | MB_OKCANCEL,
                                ) == IDOK
                            }
                        };


                        if !should_update {
                            log::info!("用户取消了更新");
                            return;
                        }
                        
                        // 2. 显示进度对话框并开始下载
                        if let Some(download_url) = result.download_url {
                            let proxy_url_clone2 = proxy_url.clone();
                            let proxy_username_clone2 = proxy_username.clone();
                            let proxy_password_clone2 = proxy_password.clone();
                            slint::invoke_from_event_loop(move || {
                                // 在主线程创建并显示进度框
                                if let Ok(dialog) = crate::slint_ui::UpdateProgressDialog::new() {
                                    #[cfg(target_os = "macos")]
                                    dialog.set_text_font("Heiti SC".into());
                                    #[cfg(target_os = "windows")]
                                    dialog.set_text_font("Microsoft YaHei".into());

                                    dialog.set_status_text("准备下载...".into());
                                    dialog.set_progress(0.0);
                                    dialog.set_can_cancel(true);
                                    
                                    let dialog_weak = dialog.as_weak();
                                    
                                    // 标记是否取消下载
                                    let is_cancelled = Arc::new(AtomicBool::new(false));
                                    let is_cancelled_clone = is_cancelled.clone();
                                    
                                    dialog.on_cancel(move || {
                                        is_cancelled_clone.store(true, Ordering::SeqCst);
                                        // 立即隐藏对话框，无需等待后台线程结束
                                        if let Some(d) = dialog_weak.upgrade() {
                                            let _ = d.hide();
                                        }
                                    });
                                    
                                    let _ = dialog.show();
                                    
                                    // 共享状态供后台线程和UI线程通讯 (progress, text, can_cancel, finished)
                                    let progress_state = Arc::new(std::sync::Mutex::new((0.0f32, String::new(), true, false)));
                                    
                                    let timer_rc = std::rc::Rc::new(std::cell::RefCell::new(Some(slint::Timer::default())));
                                    let timer_clone = timer_rc.clone();
                                    
                                    let progress_state_timer = progress_state.clone();
                                    let is_cancelled_timer = is_cancelled.clone();
                                    let d_weak_timer = dialog.as_weak();
                                    
                                    timer_rc.borrow().as_ref().unwrap().start(slint::TimerMode::Repeated, std::time::Duration::from_millis(100), move || {
                                        let mut finished = false;
                                        let is_cancelled_now = is_cancelled_timer.load(Ordering::SeqCst);

                                        if let Some(d) = d_weak_timer.upgrade() {
                                            let (pct, text, can_cancel, is_done) = {
                                                let state = progress_state_timer.lock().unwrap();
                                                (state.0, state.1.clone(), state.2, state.3)
                                            };

                                            // 取消进行中时不覆盖 UI 文字（保持"正在取消..."）
                                            if !is_cancelled_now && !text.is_empty() {
                                                d.set_status_text(text.into());
                                                d.set_progress(pct);
                                                d.set_can_cancel(can_cancel);
                                            }

                                            if is_done {
                                                finished = true;
                                                if is_cancelled_now {
                                                    // 取消完成后直接隐藏对话框
                                                    let _ = d.hide();
                                                }
                                                // 非取消情况下（成功/失败），对话框保持显示
                                                // 让用户看到最终状态文字（如"应用更新失败"）
                                            }
                                        } else {
                                            finished = true;
                                        }

                                        if finished {
                                            if let Ok(mut t) = timer_clone.try_borrow_mut() {
                                                *t = None;
                                            }
                                        }
                                    });
                                    
                                    // 3. 在新线程中执行下载及解压
                                    let progress_state_thread = progress_state.clone();
                                    let is_cancelled_thread = is_cancelled.clone();
                                    std::thread::spawn(move || {
                                        let rt2 = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                                        rt2.block_on(async move {
                                            let extract_dir = std::env::temp_dir().join("screenhop_update");
                                            let _ = std::fs::remove_dir_all(&extract_dir); // 清理旧缓存
                                            std::fs::create_dir_all(&extract_dir).unwrap();
                                            
                                            log::info!("开始下载并解压到: {:?}", extract_dir);
                                            
                                            // 定义进度回调
                                            let progress_state_cb = progress_state_thread.clone();
                                            let progress_cb = move |downloaded: u64, total: u64| {
                                                let pct = if total > 0 { downloaded as f32 / total as f32 } else { 0.0 };
                                                let mb_downloaded = downloaded as f32 / 1024.0 / 1024.0;
                                                let mb_total = total as f32 / 1024.0 / 1024.0;
                                                
                                                let text = format!("正在下载: {:.1} MB / {:.1} MB", mb_downloaded, mb_total);
                                                let mut state = progress_state_cb.lock().unwrap();
                                                state.0 = pct;
                                                state.1 = text;
                                            };
                                            
                                            // 执行下载和解压
                                            let dl_res = screenhop_core::updater::download_and_extract(
                                                &download_url,
                                                &extract_dir,
                                                proxy_url_clone2.as_deref(),
                                                proxy_username_clone2.as_deref(),
                                                proxy_password_clone2.as_deref(),
                                                progress_cb
                                            ).await;
                                            
                                            if is_cancelled_thread.load(Ordering::SeqCst) {
                                                log::info!("用户已取消更新");
                                                let mut state = progress_state_thread.lock().unwrap();
                                                state.3 = true; // finish
                                                return;
                                            }
                                            
                                            match dl_res {
                                                Ok(_) => {
                                                    // 4. 下载解压成功，执行应用替换逻辑
                                                    {
                                                        let mut state = progress_state_thread.lock().unwrap();
                                                        state.1 = "正在安装更新并重启...".to_string();
                                                        state.0 = 1.0;
                                                        state.2 = false;
                                                    }
                                                    
                                                    // 应用更新
                                                    #[cfg(target_os = "macos")]
                                                    let apply_res = screenhop_core::updater::apply_update_macos(&extract_dir);
                                                    
                                                    #[cfg(target_os = "windows")]
                                                    let apply_res = screenhop_core::updater::apply_update_windows(&extract_dir);
                                                    
                                                    if let Err(e) = apply_res {
                                                        log::error!("应用更新失败: {:?}", e);
                                                        let mut state = progress_state_thread.lock().unwrap();
                                                        state.1 = format!("应用更新失败: {}", e);
                                                        state.2 = true;
                                                        state.3 = true;
                                                    } else {
                                                        log::info!("更新应用成功，准备退出当前进程");
                                                        std::process::exit(0);
                                                    }
                                                },
                                                Err(e) => {
                                                    log::error!("下载解压失败: {:?}", e);
                                                    let mut state = progress_state_thread.lock().unwrap();
                                                    state.1 = format!("下载失败: {}", e);
                                                    state.2 = true;
                                                    state.3 = true;
                                                }
                                            }
                                        });
                                    });
                                }
                            }).unwrap();
                        } else {
                            log::error!("未找到可用的下载链接");
                        }
                    } else {
                        log::info!("当前已是最新版本 ({})", result.current_version);
                        if manual {
                            #[cfg(target_os = "macos")]
                            {
                                let script = format!(
                                    "display dialog \"当前已是最新版本 ({})\" with title \"ScreenHop 更新\" buttons {{\"确定\"}} default button 1",
                                    result.current_version
                                );
                                let _ = std::process::Command::new("osascript").arg("-e").arg(&script).output();
                            }
                            
                            #[cfg(target_os = "windows")]
                            {
                                use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONINFORMATION, MB_OK};
                                use windows::core::{HSTRING, PCWSTR};
                                
                                let text = format!("当前已是最新版本 ({})", result.current_version);
                                let title = "ScreenHop 更新";
                                
                                let h_text = HSTRING::from(text);
                                let h_title = HSTRING::from(title);
                                
                                unsafe {
                                    MessageBoxW(
                                        None,
                                        PCWSTR(h_text.as_ptr()),
                                        PCWSTR(h_title.as_ptr()),
                                        MB_ICONINFORMATION | MB_OK,
                                    );
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("检查更新失败: {}", e);
                    if manual {
                        #[cfg(target_os = "macos")]
                        {
                            let script = format!(
                                "display dialog \"检查更新失败: {}\" with title \"ScreenHop 更新\" buttons {{\"确定\"}} default button 1 with icon stop",
                                e
                            );
                            let _ = std::process::Command::new("osascript").arg("-e").arg(&script).output();
                        }
                    }
                }
            }
        });
    });
}



struct MenuItems {
    toggle_item: MenuItem,
    autostart_item: MenuItem,
    auto_check_update_item: MenuItem,
    proxy_enable_item: MenuItem,
}

/// 运行托盘应用主循环
pub fn run_app(config: AppConfig) -> Result<()> {
    // 我们现在使用 slint::run_event_loop_until_quit()，不再需要隐藏窗口来维持事件循环
    
    let config = Arc::new(Mutex::new(config));

    // 创建菜单
    let menu = Menu::new();

    let status_item = MenuItem::new("Window Mover is Running", false, None);
    let toggle_item = MenuItem::with_id(
        MENU_ID_TOGGLE,
        if config.lock().unwrap().disable_hook {
            "启用鼠标中键移动"
        } else {
            "禁用鼠标中键移动"
        },
        true,
        None,
    );
    let autostart_item = MenuItem::with_id(
        MENU_ID_AUTOSTART,
        if config.lock().unwrap().auto_start {
            "✓ 开机自动启动"
        } else {
            "  开机自动启动"
        },
        true,
        None,
    );
    let update_item = MenuItem::with_id(MENU_ID_CHECK_UPDATE, "检查更新", true, None);
    let auto_check_update_item = MenuItem::with_id(
        MENU_ID_AUTO_CHECK_UPDATE,
        if config.lock().unwrap().auto_check_update {
            "✓ 启动时检查更新"
        } else {
            "  启动时检查更新"
        },
        true,
        None,
    );
    
    let proxy_menu = muda::Submenu::new("代理设置", true);
    let proxy_enable_item = MenuItem::with_id(
        MENU_ID_PROXY_ENABLE,
        {
            let cfg = config.lock().unwrap();
            if cfg.proxy_enabled {
                "✓ 启用代理"
            } else {
                "  启用代理"
            }
        },
        true,
        None,
    );
    let proxy_settings_item = MenuItem::with_id(
        MENU_ID_PROXY_SETTINGS,
        "代理设置...",
        true,
        None,
    );
    proxy_menu.append(&proxy_enable_item).ok();
    proxy_menu.append(&proxy_settings_item).ok();

    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::with_id(MENU_ID_QUIT, "退出", true, None);

    menu.append(&status_item).ok();
    menu.append(&separator).ok();
    menu.append(&toggle_item).ok();
    menu.append(&autostart_item).ok();
    menu.append(&update_item).ok();
    menu.append(&auto_check_update_item).ok();
    menu.append(&proxy_menu).ok();
    menu.append(&PredefinedMenuItem::separator()).ok();
    menu.append(&quit_item).ok();

    // 创建托盘图标
    let icon = create_tray_icon_image();
    let _tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Window Mover")
        .with_icon(icon)
        .build()?;

    log::info!("系统托盘图标已创建");

    // 启动时自动检查更新
    {
        let cfg = config.lock().unwrap();
        if cfg.auto_check_update {
            log::info!("启动时自动检查更新...");
            let (proxy_url, proxy_username, proxy_password) = if cfg.proxy_enabled && !cfg.proxy_url.is_empty() {
                (Some(cfg.proxy_url.clone()), cfg.proxy_username.clone(), cfg.proxy_password.clone())
            } else {
                (None, None, None)
            };
            do_check_update(env!("CARGO_PKG_VERSION"), proxy_url, proxy_username, proxy_password, false);
        }
    }

    // 在主线程处理 Slint 和 MenuEvents
    let menu_items = MenuItems {
        toggle_item,
        autostart_item,
        auto_check_update_item,
        proxy_enable_item,
    };
    let config_clone = config.clone();


    #[cfg(target_os = "macos")]
    let mut macos_policy_set = false;

    // 定时检查 Menu事件
    let timer = slint::Timer::default();
    timer.start(slint::TimerMode::Repeated, std::time::Duration::from_millis(50), move || {
        // macOS: 在事件循环刚启动时将 NSApplication 激活策略设为 Accessory（不在 Dock 显示）
        // 以覆盖 slint/winit 默认强设为 Regular 的行为
        #[cfg(target_os = "macos")]
        if !macos_policy_set {
            #[allow(deprecated)]
            unsafe {
                use cocoa::appkit::NSApp;
                use objc::*;
                let app = NSApp();
                let _: () = msg_send![app, setActivationPolicy: 1i64]; // Accessory
                
                // Set the application icon
                let icon_bytes = include_bytes!("../../../assets/icon-256.png");
                let data = cocoa::foundation::NSData::dataWithBytes_length_(
                    cocoa::base::nil,
                    icon_bytes.as_ptr() as *const std::ffi::c_void,
                    icon_bytes.len() as u64,
                );
                let ns_image: cocoa::base::id = msg_send![class!(NSImage), alloc];
                let ns_image: cocoa::base::id = msg_send![ns_image, initWithData: data];
                if ns_image != cocoa::base::nil {
                    let _: () = msg_send![app, setApplicationIconImage: ns_image];
                }
                
                // 强制解除此后台应用对前台的抢占，把焦点还给此前的应用
                // 修复首次点出托盘菜单时当前工作窗口突然失去焦点的 Bug
                // (winit在 cargo run 期间会强制把我们变成 Regular，所以这步补救是必须的)
                let _: () = msg_send![app, deactivate];
            }
            macos_policy_set = true;
        }

        while let Ok(event) = MenuEvent::receiver().try_recv() {
            if event.id.0 == MENU_ID_QUIT {
                std::process::exit(0);
            }
            handle_menu_event(&event.id.0, &menu_items, &config_clone);
        }
    });
    slint::run_event_loop_until_quit()?;
    Ok(())
}

fn handle_menu_event(id: &str, items: &MenuItems, config: &Arc<Mutex<AppConfig>>) {
    match id {
        MENU_ID_TOGGLE => {
            if let Ok(mut cfg) = config.lock() {
                cfg.disable_hook = !cfg.disable_hook;
                let state = if cfg.disable_hook { "已禁用" } else { "已启用" };
                log::info!("鼠标中键移动功能{}", state);
                
                let text = if cfg.disable_hook {
                    "启用鼠标中键移动"
                } else {
                    "禁用鼠标中键移动"
                };
                items.toggle_item.set_text(text);

                // 实时启用/禁用钩子（无需重启）
                crate::engine::set_hook_enabled(!cfg.disable_hook);

                if let Err(e) = cfg.save() {
                    log::error!("保存配置失败: {}", e);
                }
            }
        }
        MENU_ID_AUTOSTART => {
            if let Ok(mut cfg) = config.lock() {
                cfg.auto_start = !cfg.auto_start;
                log::info!("开机自启动: {}", cfg.auto_start);

                let text = if cfg.auto_start {
                    "✓ 开机自动启动"
                } else {
                    "  开机自动启动"
                };
                items.autostart_item.set_text(text);

                #[cfg(target_os = "macos")]
                {
                    use screenhop_platform::AutoStart;
                    let auto = screenhop_platform::macos::autostart::MacAutoStart::new();
                    if let Err(e) = auto.set_enabled(cfg.auto_start) {
                        log::error!("设置自启动失败: {}", e);
                    }
                }

                if let Err(e) = cfg.save() {
                    log::error!("保存配置失败: {}", e);
                }
            }
        }
        MENU_ID_CHECK_UPDATE => {
            log::info!("手动检查更新...");
            let (proxy_url, proxy_username, proxy_password) = if let Ok(cfg) = config.lock() {
                if cfg.proxy_enabled && !cfg.proxy_url.is_empty() {
                    (Some(cfg.proxy_url.clone()), cfg.proxy_username.clone(), cfg.proxy_password.clone())
                } else {
                    (None, None, None)
                }
            } else {
                (None, None, None)
            };
            do_check_update(env!("CARGO_PKG_VERSION"), proxy_url, proxy_username, proxy_password, true);
        }
        MENU_ID_AUTO_CHECK_UPDATE => {
            if let Ok(mut cfg) = config.lock() {
                cfg.auto_check_update = !cfg.auto_check_update;
                log::info!("启动时检查更新: {}", cfg.auto_check_update);

                let text = if cfg.auto_check_update {
                    "✓ 启动时检查更新"
                } else {
                    "  启动时检查更新"
                };
                items.auto_check_update_item.set_text(text);

                if let Err(e) = cfg.save() {
                    log::error!("保存配置失败: {}", e);
                }
            }
        }
        MENU_ID_PROXY_ENABLE => {
            if let Ok(mut cfg) = config.lock() {
                if cfg.proxy_enabled {
                    cfg.proxy_enabled = false;
                    log::info!("代理已禁用");
                    items.proxy_enable_item.set_text("  启用代理");
                    if let Err(e) = cfg.save() {
                        log::error!("保存配置失败: {}", e);
                    }
                } else {
                    if cfg.proxy_url.is_empty() {
                        cfg.proxy_url = "socks5://127.0.0.1:2888".to_string();
                        log::info!("未配置代理地址，自动填充为默认: {}", cfg.proxy_url);
                    }
                    cfg.proxy_enabled = true;
                    log::info!("代理已启用");
                    items.proxy_enable_item.set_text("✓ 启用代理");
                    if let Err(e) = cfg.save() {
                        log::error!("保存配置失败: {}", e);
                    }
                }
            }
        }
        MENU_ID_PROXY_SETTINGS => {
            let (mut address, mut port, mut protocol, mut username, mut password, mut enabled) = (
                "127.0.0.1".to_string(), "2888".to_string(), "SOCKS5".to_string(),
                String::new(), String::new(), false
            );
            
            if let Ok(cfg) = config.lock() {
                enabled = cfg.proxy_username.is_some();
                username = cfg.proxy_username.clone().unwrap_or_default();
                password = cfg.proxy_password.clone().unwrap_or_default();
                
                if !cfg.proxy_url.is_empty() {
                    let url = &cfg.proxy_url;
                    let (scheme, rest) = url.split_once("://").unwrap_or(("socks5", url));
                    
                    protocol = match scheme.to_lowercase().as_str() {
                        "socks4" | "socks4a" => "SOCKS4",
                        "https" => "HTTPS",
                        "http" => "HTTP",
                        _ => "SOCKS5",
                    }.to_string();
                    
                    if let Some((addr, p)) = rest.split_once(':') {
                        address = addr.to_string();
                        port = p.to_string();
                    } else {
                        address = rest.to_string();
                    }
                }
            }
            
            let dialog = crate::slint_ui::ProxyAuthDialog::new().unwrap();
            
            #[cfg(target_os = "windows")]
            dialog.set_text_font("Microsoft YaHei".into());
            
            dialog.set_address(address.into());
            dialog.set_port(port.into());
            dialog.set_protocol(protocol.into());
            dialog.set_auth_enabled(enabled);
            dialog.set_username(username.into());
            dialog.set_password(password.into());
            
            let dialog_weak2 = dialog.as_weak();
            dialog.on_cancel(move || {
                if let Some(d) = dialog_weak2.upgrade() {
                    let _ = d.hide();
                }
            });

            let dialog_weak = dialog.as_weak();
            let config_clone = config.clone();
            let proxy_enable_item_clone = items.proxy_enable_item.clone();
            
            dialog.on_apply(move |addr, p, proto: slint::SharedString, auth_en: bool, user: slint::SharedString, pass: slint::SharedString| {
                let proxy_url_str = match proto.as_str() {
                    "SOCKS5" => format!("socks5://{}:{}", addr, p),
                    "SOCKS4" => format!("socks4://{}:{}", addr, p),
                    "HTTPS" => format!("https://{}:{}", addr, p),
                    "HTTP" => format!("http://{}:{}", addr, p),
                    _ => format!("socks5://{}:{}", addr, p),
                };

                if let Ok(mut cfg) = config_clone.lock() {
                    cfg.proxy_url = proxy_url_str.clone();
                    cfg.proxy_enabled = true;
                    if auth_en && !user.is_empty() {
                        cfg.proxy_username = Some(user.into());
                        cfg.proxy_password = Some(pass.into());
                    } else {
                        cfg.proxy_username = None;
                        cfg.proxy_password = None;
                    }
                    
                    log::info!("代理地址及认证已配置: {}", cfg.proxy_url);
                    proxy_enable_item_clone.set_text("✓ 启用代理");
                    
                    if let Err(e) = cfg.save() {
                        log::error!("保存配置失败: {}", e);
                    }
                }
                if let Some(d) = dialog_weak.upgrade() {
                    let _ = d.hide();
                }
            });
            
            let _ = dialog.show();
            
            // macOS: 强制激活当前应用并将其置于前台，解决第一次启动不聚焦的问题
            #[cfg(target_os = "macos")]
            #[allow(deprecated)]
            unsafe {
                use cocoa::appkit::NSApp;
                use objc::*;
                let app = NSApp();
                let _: () = msg_send![app, activateIgnoringOtherApps: true];
            }
        }
        MENU_ID_QUIT => {
            log::info!("用户请求退出");
            std::process::exit(0);
        }
        _ => {}
    }
}
