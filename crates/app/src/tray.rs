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

/// 在后台线程中执行更新检查
fn do_check_update(
    version: &str,
    proxy_url: Option<String>,
    proxy_username: Option<String>,
    proxy_password: Option<String>,
    manual: bool,
) {
    let version = version.to_string();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        rt.block_on(async {
            match screenhop_core::updater::check_for_update(&version, proxy_url.as_deref(), proxy_username.as_deref(), proxy_password.as_deref()).await {
                Ok(result) => {
                    if result.has_update {
                        log::info!(
                            "发现新版本: {} → {}",
                            result.current_version,
                            result.latest_version
                        );
                        if manual {
                            #[cfg(target_os = "macos")]
                            {
                                let script = format!(
                                    "display dialog \"发现新版本: {} \\n是否前往下载？\" with title \"ScreenHop 更新\" buttons {{\"取消\", \"前往下载\"}} default button 2",
                                    result.latest_version
                                );
                                if let Ok(out) = std::process::Command::new("osascript").arg("-e").arg(&script).output() {
                                    if String::from_utf8_lossy(&out.stdout).contains("前往下载") {
                                        let _ = open::that(&result.release_url);
                                    }
                                }
                            }
                            #[cfg(not(target_os = "macos"))]
                            {
                                let _ = open::that(&result.release_url);
                            }
                        } else {
                            if let Err(e) = open::that(&result.release_url) {
                                log::error!("打开浏览器失败: {}", e);
                            }
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
        MENU_ID_PROXY_ENABLE => {
            if let Ok(mut cfg) = config.lock() {
                if cfg.proxy_enabled {
                    cfg.proxy_enabled = false;
                    log::info!("代理已禁用");
                    items.proxy_enable_item.set_text("  启用代理");
                    if let Err(e) = cfg.save() {
                        log::error!("保存配置失败: {}", e);
                    }
                } else if !cfg.proxy_url.is_empty() {
                    cfg.proxy_enabled = true;
                    log::info!("代理已启用");
                    items.proxy_enable_item.set_text("✓ 启用代理");
                    if let Err(e) = cfg.save() {
                        log::error!("保存配置失败: {}", e);
                    }
                } else {
                    // URL 未配置，提示打开设置
                    log::info!("请先在代理设置中配置代理地址");
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
