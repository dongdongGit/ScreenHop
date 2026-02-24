#![allow(unexpected_cfgs)]

mod engine;
mod tray;

use anyhow::{Context, Result};
use single_instance::SingleInstance;
use screenhop_core::config::AppConfig;

const APP_ID: &str = "com.dongdong.screenhop";
const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> Result<()> {
    // 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format_timestamp_millis()
        .init();

    log::info!("ScreenHop v{} 启动中...", APP_VERSION);

    // 单实例检测
    let instance = SingleInstance::new(APP_ID).context("单实例检测失败")?;

    if !instance.is_single() {
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

        // 检查辅助功能权限
        if !platform.check_accessibility_permissions() {
            log::error!("缺少辅助功能权限，请在系统设置中授权");
        }

        // 安装鼠标中键事件钩子
        if !config.disable_hook {
            engine::install_hook(&config)?;
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
