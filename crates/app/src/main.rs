#![allow(unexpected_cfgs)]
#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

mod engine;
mod slint_ui;
mod tray;

use anyhow::{Context, Result};
use screenhop_core::config::AppConfig;
use std::net::TcpListener;

#[allow(dead_code)]
const APP_ID: &str = "com.dongdong.screenhop";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// 使用 TCP 端口锁实现单实例检测（避免 single-instance crate 在 .app 中的文件系统只读问题）
fn try_lock_single_instance() -> bool {
    // 绑定一个固定的本地端口；成功则说明当前是唯一实例
    match TcpListener::bind("127.0.0.1:57832") {
        Ok(listener) => {
            // 把监听器泄露到堆上，让它在进程退出前一直持有端口
            Box::leak(Box::new(listener));
            true
        }
        Err(_) => false, // 端口被占用，说明已有实例在运行
    }
}

fn inner_main() -> Result<()> {
    // 单实例检测
    if !try_lock_single_instance() {
        log::warn!("程序已在运行中，退出");
        return Ok(());
    }

    // 加载配置
    let config = AppConfig::load().context("加载配置失败")?;
    log::info!("配置已加载: {:?}", config);

    // macOS: 检查权限 + 安装事件钩子 + 启动托盘
    #[cfg(target_os = "macos")]
    {
        let platform = screenhop_platform::create_platform();

        let has_perm = platform.check_accessibility_permissions();
        if !has_perm {
            log::warn!("缺少辅助功能权限，正在触发系统授权提示...");
            // 触发 macOS 系统权限请求对话框
            platform.request_accessibility_permissions();

            // 为了防止系统自带弹窗没弹出来（macOS 经常吞弹窗），加上我们自己的弹窗引导
            let script = r#"
                display alert "ScreenHop 需要「辅助功能」权限" message "ScreenHop 需要该权限来监听鼠标中键移动窗口。\n\n请在随后的系统设置中，找到并勾选 ScreenHop。" buttons {"前往授权", "稍后"} default button 1
                if button returned of result is "前往授权" then
                    tell application "System Settings"
                        activate
                        reveal anchor "Privacy_Accessibility" of pane id "com.apple.settings.PrivacySecurity.extension"
                    end tell
                    
                    -- Fallback for older macOS versions
                    tell application "System Preferences"
                        activate
                        reveal anchor "Privacy_Accessibility" of pane id "com.apple.preference.security"
                    end tell
                end if
            "#;
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(script)
                .output();

            // 权限不足时跳过钩子安装，但继续运行展示托盘图标
            log::warn!("已跳过鼠标钩子安装（缺少权限），请授权后重启 ScreenHop");
        } else {
            // 安装鼠标中键事件钩子
            if !config.disable_hook {
                engine::install_hook(&config)?;
            }
        }

        // 启动系统托盘 + NSApp 事件循环（阻塞）
        tray::run_app(config)?;
    }

    // Windows: 安装钩子 + 启动托盘 + 消息循环
    #[cfg(target_os = "windows")]
    {
        // 安装鼠标中键事件钩子
        if !config.disable_hook {
            engine::install_hook(&config)?;
        }

        // 启动系统托盘 + Windows 消息循环（阻塞）
        tray::run_app(config)?;
    }

    log::info!("ScreenHop 已退出");
    Ok(())
}

fn main() {
    // 获取系统的临时目录
    let temp_dir = std::env::temp_dir();
    let run_log_path = temp_dir.join("screenhop_run.log");
    let err_log_path = temp_dir.join("screenhop_fatal_err.log");

    // 初始化日志到文件，方便 Finder 启动时或隐藏执行时调试
    let log_file = std::fs::File::create(&run_log_path).unwrap();
    let target = Box::new(log_file);
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .target(env_logger::Target::Pipe(target))
        .init();

    log::info!("ScreenHop v{} 启动中...", APP_VERSION);

    if let Err(e) = inner_main() {
        log::error!("致命错误导致应用退出: {:?}", e);
        std::fs::write(&err_log_path, format!("{:?}", e)).ok();
    }
}
