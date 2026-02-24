use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    /// 是否禁用鼠标中键移动功能
    #[serde(default)]
    pub disable_hook: bool,

    /// 是否开机自动启动
    #[serde(default)]
    pub auto_start: bool,

    /// 是否总是最小化启动（隐藏至系统托盘）
    #[serde(default)]
    pub start_minimized: bool,

    /// 是否启动时自动检查更新
    #[serde(default = "default_true")]
    pub auto_check_update: bool,

    /// 标题栏检测高度（像素）
    #[serde(default = "default_title_bar_height")]
    pub title_bar_height: f64,

    /// 是否启用代理
    #[serde(default)]
    pub proxy_enabled: bool,

    /// 代理地址（如 http://127.0.0.1:7890、socks5://127.0.0.1:1080）
    #[serde(default)]
    pub proxy_url: String,

    /// 代理用户名（可选）
    #[serde(default)]
    pub proxy_username: Option<String>,

    /// 代理密码（可选）
    #[serde(default)]
    pub proxy_password: Option<String>,
}

fn default_true() -> bool {
    true
}

fn default_title_bar_height() -> f64 {
    40.0
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            disable_hook: false,
            auto_start: false,
            start_minimized: false,
            auto_check_update: true,
            title_bar_height: default_title_bar_height(),
            proxy_enabled: false,
            proxy_url: String::new(),
            proxy_username: None,
            proxy_password: None,
        }
    }
}

impl AppConfig {
    /// 获取配置文件路径
    /// - macOS: ~/Library/Application Support/screenhop/config.toml
    /// - Windows: %APPDATA%/screenhop/config.toml
    pub fn config_path() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("无法获取配置目录")?
            .join("screenhop");

        fs::create_dir_all(&config_dir)
            .context("无法创建配置目录")?;

        Ok(config_dir.join("config.toml"))
    }

    /// 从配置文件加载，如果文件不存在则返回默认配置
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("无法读取配置文件: {}", path.display()))?;

        let config: Self = toml::from_str(&content)
            .with_context(|| "配置文件格式错误")?;

        Ok(config)
    }

    /// 保存配置到文件
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        let content = toml::to_string_pretty(self)
            .context("配置序列化失败")?;

        fs::write(&path, content)
            .with_context(|| format!("无法写入配置文件: {}", path.display()))?;

        log::debug!("配置已保存到: {}", path.display());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AppConfig::default();
        assert!(!config.disable_hook);
        assert!(!config.auto_start);
        assert!(!config.start_minimized);
        assert!(config.auto_check_update);
        assert_eq!(config.title_bar_height, 40.0);
        assert!(!config.proxy_enabled);
        assert!(config.proxy_url.is_empty());
        assert!(config.proxy_username.is_none());
        assert!(config.proxy_password.is_none());
    }

    #[test]
    fn test_serialize_deserialize() {
        let config = AppConfig::default();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        let loaded: AppConfig = toml::from_str(&toml_str).unwrap();
        assert_eq!(config.disable_hook, loaded.disable_hook);
        assert_eq!(config.auto_start, loaded.auto_start);
    }
}
