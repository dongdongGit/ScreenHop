use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;

const GITHUB_API_URL: &str =
    "https://api.github.com/repos/dongdongGit/ScreenHop/releases/latest";
const RELEASES_PAGE_URL: &str =
    "https://github.com/dongdongGit/ScreenHop/releases/latest";

/// 更新检查结果
#[derive(Debug, Clone)]
pub struct UpdateCheckResult {
    pub has_update: bool,
    pub latest_version: String,
    pub current_version: String,
    pub release_url: String,
    pub download_url: Option<String>,
    pub asset_name: Option<String>,
    pub asset_size: u64,
    pub error_message: Option<String>,
}

/// GitHub Release API 响应结构
#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    html_url: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

/// 获取当前平台的资源关键字
fn get_platform_keyword() -> &'static str {
    if cfg!(target_os = "windows") {
        match std::env::consts::ARCH {
            "x86_64" => "win-x64",
            "x86" => "win-x86",
            "aarch64" => "win-arm64",
            _ => "win-x64",
        }
    } else if cfg!(target_os = "macos") {
        match std::env::consts::ARCH {
            "aarch64" => "macOS-arm64",
            "x86_64" => "macOS-x86_64",
            _ => "macOS-universal",
        }
    } else {
        "unknown"
    }
}

/// 从 assets 列表中找到匹配当前平台的下载链接
fn find_matching_asset(assets: &[GithubAsset]) -> Option<&GithubAsset> {
    let platform_key = get_platform_keyword();
    log::debug!("查找匹配资源: platform={}", platform_key);

    // 精确匹配平台关键字
    assets.iter().find(|a| {
        a.name.contains(platform_key) && a.name.ends_with(".zip")
    })
}

/// 检查是否有新版本
pub async fn check_for_update(current_version: &str) -> Result<UpdateCheckResult> {
    let mut result = UpdateCheckResult {
        has_update: false,
        latest_version: String::new(),
        current_version: current_version.to_string(),
        release_url: RELEASES_PAGE_URL.to_string(),
        download_url: None,
        asset_name: None,
        asset_size: 0,
        error_message: None,
    };

    let client = reqwest::Client::builder()
        .user_agent("ScreenHop-UpdateChecker")
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .context("创建 HTTP 客户端失败")?;

    let response = client
        .get(GITHUB_API_URL)
        .send()
        .await
        .context("请求 GitHub API 失败")?;

    if !response.status().is_success() {
        result.error_message = Some(format!("GitHub API 返回 {}", response.status()));
        return Ok(result);
    }

    let release: GithubRelease = response
        .json()
        .await
        .context("解析 GitHub API 响应失败")?;

    let latest_str = release.tag_name.trim_start_matches('v');
    result.latest_version = latest_str.to_string();
    result.release_url = release.html_url;

    // 版本比较
    if let (Ok(latest), Ok(current)) = (
        Version::parse(latest_str),
        Version::parse(current_version),
    ) {
        result.has_update = latest > current;
    } else {
        result.has_update = latest_str != current_version;
    }

    // 查找匹配的下载资源
    if result.has_update {
        if let Some(asset) = find_matching_asset(&release.assets) {
            result.download_url = Some(asset.browser_download_url.clone());
            result.asset_name = Some(asset.name.clone());
            result.asset_size = asset.size;
        }
    }

    Ok(result)
}

/// 下载文件并报告进度
pub async fn download_file<F>(
    url: &str,
    dest_path: &std::path::Path,
    progress_callback: F,
) -> Result<()>
where
    F: Fn(u64, u64),
{
    let client = reqwest::Client::builder()
        .user_agent("ScreenHop-UpdateChecker")
        .build()
        .context("创建 HTTP 客户端失败")?;

    let response = client
        .get(url)
        .send()
        .await
        .context("下载请求失败")?;

    let total_size = response.content_length().unwrap_or(0);
    let bytes = response.bytes().await.context("读取下载内容失败")?;
    progress_callback(bytes.len() as u64, total_size);

    std::fs::write(dest_path, &bytes)
        .context("写入下载文件失败")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_platform_keyword() {
        let keyword = get_platform_keyword();
        // 在 macOS 上应该返回 macOS 相关的关键字
        #[cfg(target_os = "macos")]
        assert!(keyword.starts_with("macOS"));
        #[cfg(target_os = "windows")]
        assert!(keyword.starts_with("win"));
    }
}
