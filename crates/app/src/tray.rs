use anyhow::Result;
use muda::{Menu, MenuItem, PredefinedMenuItem};
use tray_icon::{menu::MenuEvent, Icon, TrayIconBuilder};
use screenhop_core::config::AppConfig;

use std::sync::{Arc, Mutex};

const MENU_ID_TOGGLE: &str = "toggle";
const MENU_ID_AUTOSTART: &str = "autostart";
const MENU_ID_CHECK_UPDATE: &str = "check_update";
const MENU_ID_QUIT: &str = "quit";

/// 创建托盘图标（使用简单的纯色图标）
fn create_tray_icon_image() -> Icon {
    // 创建一个 32x32 的蓝色圆形图标
    let size = 32u32;
    let mut rgba = vec![0u8; (size * size * 4) as usize];

    let center = size as f64 / 2.0;
    let radius = center - 2.0;

    for y in 0..size {
        for x in 0..size {
            let dx = x as f64 - center;
            let dy = y as f64 - center;
            let dist = (dx * dx + dy * dy).sqrt();

            let idx = ((y * size + x) * 4) as usize;
            if dist <= radius {
                // 蓝色圆形
                rgba[idx] = 0; // R
                rgba[idx + 1] = 120; // G
                rgba[idx + 2] = 215; // B
                rgba[idx + 3] = 255; // A
            } else {
                // 透明
                rgba[idx] = 0;
                rgba[idx + 1] = 0;
                rgba[idx + 2] = 0;
                rgba[idx + 3] = 0;
            }
        }
    }

    Icon::from_rgba(rgba, size, size).expect("创建图标失败")
}

/// 运行托盘应用主循环
pub fn run_app(config: AppConfig) -> Result<()> {
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
    let separator = PredefinedMenuItem::separator();
    let quit_item = MenuItem::with_id(MENU_ID_QUIT, "退出", true, None);

    menu.append(&status_item).ok();
    menu.append(&separator).ok();
    menu.append(&toggle_item).ok();
    menu.append(&autostart_item).ok();
    menu.append(&update_item).ok();
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

    // macOS: 使用 NSApplication 事件循环
    #[cfg(target_os = "macos")]
    {
        #[allow(deprecated)]
        unsafe {
            use cocoa::appkit::NSApp;
            use objc::*;

            let app = NSApp();
            let _: () = msg_send![app, setActivationPolicy: 1i64]; // NSApplicationActivationPolicyAccessory

            // 在单独的线程中处理菜单事件
            let config_clone = config.clone();
            std::thread::spawn(move || loop {
                if let Ok(event) = MenuEvent::receiver().recv() {
                    handle_menu_event(&event.id.0, &config_clone);
                }
            });

            let _: () = msg_send![app, run];
        }
    }

    // Windows: 简单的事件循环
    #[cfg(target_os = "windows")]
    {
        loop {
            if let Ok(event) = MenuEvent::receiver().recv() {
                if event.id.0 == MENU_ID_QUIT {
                    break;
                }
                handle_menu_event(&event.id.0, &config);
            }
        }
    }

    Ok(())
}

fn handle_menu_event(id: &str, config: &Arc<Mutex<AppConfig>>) {
    match id {
        MENU_ID_TOGGLE => {
            if let Ok(mut cfg) = config.lock() {
                cfg.disable_hook = !cfg.disable_hook;
                let state = if cfg.disable_hook { "已禁用" } else { "已启用" };
                log::info!("鼠标中键移动功能{}", state);
                if let Err(e) = cfg.save() {
                    log::error!("保存配置失败: {}", e);
                }
            }
        }
        MENU_ID_AUTOSTART => {
            if let Ok(mut cfg) = config.lock() {
                cfg.auto_start = !cfg.auto_start;
                log::info!("开机自启动: {}", cfg.auto_start);

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
            // TODO: 在后台线程中执行更新检查
        }
        MENU_ID_QUIT => {
            log::info!("用户请求退出");
            std::process::exit(0);
        }
        _ => {}
    }
}
