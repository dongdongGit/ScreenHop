use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;

const GITHUB_API_URL: &str = "https://api.github.com/repos/dongdongGit/ScreenHop/releases/latest";
const RELEASES_PAGE_URL: &str = "https://github.com/dongdongGit/ScreenHop/releases/latest";

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
    assets
        .iter()
        .find(|a| a.name.contains(platform_key) && a.name.ends_with(".zip"))
}

/// 检查是否有新版本
pub async fn check_for_update(
    current_version: &str,
    proxy_url: Option<&str>,
    proxy_username: Option<&str>,
    proxy_password: Option<&str>,
) -> Result<UpdateCheckResult> {
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

    let mut builder = reqwest::Client::builder()
        .user_agent("ScreenHop-UpdateChecker")
        .timeout(std::time::Duration::from_secs(15));

    if let Some(proxy) = proxy_url {
        let mut proxy_obj = reqwest::Proxy::all(proxy).context("代理地址格式错误")?;
        if let (Some(user), Some(pass)) = (proxy_username, proxy_password) {
            proxy_obj = proxy_obj.basic_auth(user, pass);
        }
        builder = builder.proxy(proxy_obj);
    }

    let client = builder.build().context("创建 HTTP 客户端失败")?;

    let response = client
        .get(GITHUB_API_URL)
        .send()
        .await
        .context("请求 GitHub API 失败")?;

    if !response.status().is_success() {
        result.error_message = Some(format!("GitHub API 返回 {}", response.status()));
        return Ok(result);
    }

    let release: GithubRelease = response.json().await.context("解析 GitHub API 响应失败")?;

    let latest_str = release.tag_name.trim_start_matches('v');
    result.latest_version = latest_str.to_string();
    result.release_url = release.html_url;

    // 版本比较
    if let (Ok(latest), Ok(current)) = (Version::parse(latest_str), Version::parse(current_version))
    {
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

/// 下载 zip 并解压到指定目录，通过回调报告进度 (已下载字节, 总字节)
pub async fn download_and_extract<F>(
    url: &str,
    extract_dir: &std::path::Path,
    proxy_url: Option<&str>,
    proxy_username: Option<&str>,
    proxy_password: Option<&str>,
    progress_callback: F,
) -> Result<()>
where
    F: Fn(u64, u64) + Send + 'static,
{
    use tokio::io::AsyncWriteExt;

    let mut builder = reqwest::Client::builder()
        .user_agent("ScreenHop-UpdateChecker")
        .timeout(std::time::Duration::from_secs(300));

    if let Some(proxy) = proxy_url {
        let mut proxy_obj = reqwest::Proxy::all(proxy).context("代理地址格式错误")?;
        if let (Some(user), Some(pass)) = (proxy_username, proxy_password) {
            proxy_obj = proxy_obj.basic_auth(user, pass);
        }
        builder = builder.proxy(proxy_obj);
    }

    let client = builder.build().context("创建 HTTP 客户端失败")?;

    let response = client.get(url).send().await.context("下载请求失败")?;

    let total_size = response.content_length().unwrap_or(0);

    // 写入临时 zip 文件
    let tmp_zip = extract_dir.join("update_tmp.zip");
    let mut file = tokio::fs::File::create(&tmp_zip)
        .await
        .context("创建临时文件失败")?;

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("下载数据流错误")?;
        file.write_all(&chunk).await.context("写入临时文件失败")?;
        downloaded += chunk.len() as u64;
        progress_callback(downloaded, total_size);
    }
    file.flush().await.context("刷新文件缓存失败")?;
    drop(file);

    // 解压
    log::info!("下载完成，开始解压到 {:?}", extract_dir);
    let extract_dir_owned = extract_dir.to_path_buf();
    let tmp_zip_owned = tmp_zip.clone();
    tokio::task::spawn_blocking(move || -> Result<()> {
        let f = std::fs::File::open(&tmp_zip_owned).context("打开 zip 失败")?;
        let mut archive = zip::ZipArchive::new(f).context("读取 zip 归档失败")?;
        archive
            .extract(&extract_dir_owned)
            .context("解压 zip 失败")?;
        Ok(())
    })
    .await
    .context("解压任务失败")??;

    // 删除临时 zip
    let _ = std::fs::remove_file(&tmp_zip);

    log::info!("解压完成");
    Ok(())
}

/// macOS: 将解压目录中的 .app 替换当前运行的 .app，然后重新打开
#[cfg(target_os = "macos")]
pub fn apply_update_macos(extract_dir: &std::path::Path) -> Result<()> {
    // 找到解压出的 .app
    let app_entry = std::fs::read_dir(extract_dir)
        .context("读取解压目录失败")?
        .filter_map(|e| e.ok())
        .find(|e| e.file_name().to_string_lossy().ends_with(".app"))
        .context("解压目录中未找到 .app 文件")?;

    let new_app = app_entry.path();
    log::info!("找到新版应用: {:?}", new_app);

    // 定位当前运行的 .app bundle（从 binary 向上三级: binary -> MacOS -> Contents -> .app）
    let current_exe = std::env::current_exe().context("获取当前可执行文件路径失败")?;
    // current_exe = /path/to/Foo.app/Contents/MacOS/screenhop
    let bundle_path = current_exe
        .parent() // MacOS/
        .and_then(|p| p.parent()) // Contents/
        .and_then(|p| p.parent()) // Foo.app/
        .context("无法确定当前 .app 包路径")?
        .to_path_buf();

    // 防止在 cargo run 测试时，由于向上查找三级目录而误删当前源码仓库
    if bundle_path.extension().and_then(|s| s.to_str()) != Some("app") {
        anyhow::bail!("当前运行环境不是标准的 macOS .app Bundle (推测为开发环境如 cargo run)，已跳过更新替换以防止目录损坏。");
    }

    log::info!("当前 .app 路径: {:?}", bundle_path);

    // 用 ditto 替换（ditto 能正确处理 .app bundle，保留权限和结构）
    let status = std::process::Command::new("ditto")
        .arg(&new_app)
        .arg(&bundle_path)
        .status()
        .context("执行 ditto 失败")?;

    if !status.success() {
        anyhow::bail!("ditto 替换 .app 失败，退出码: {:?}", status.code());
    }

    log::info!("替换完成，正在重新启动...");

    // 用 open 重新启动 .app
    std::process::Command::new("open")
        .arg("-a")
        .arg(&bundle_path)
        .spawn()
        .context("重新启动 .app 失败")?;

    Ok(())
}

/// Windows: 在解压目录中找到 .exe 并启动（安装程序或便携版）
#[cfg(target_os = "windows")]
pub fn apply_update_windows(extract_dir: &std::path::Path) -> Result<()> {
    // 递归查找第一个 .exe
    let exe_path = find_exe_in_dir(extract_dir).context("解压目录中未找到 .exe 文件")?;
    log::info!("找到更新程序: {:?}", exe_path);

    std::process::Command::new(&exe_path)
        .spawn()
        .context("启动更新程序失败")?;

    Ok(())
}

#[cfg(target_os = "windows")]
fn find_exe_in_dir(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                if let Some(found) = find_exe_in_dir(&path) {
                    return Some(found);
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("exe") {
                return Some(path);
            }
        }
    }
    None
}

/// 下载文件并报告进度（保留旧接口，用于兼容）
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

    let response = client.get(url).send().await.context("下载请求失败")?;

    let total_size = response.content_length().unwrap_or(0);
    let bytes = response.bytes().await.context("读取下载内容失败")?;
    progress_callback(bytes.len() as u64, total_size);

    std::fs::write(dest_path, &bytes).context("写入下载文件失败")?;

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

    /// 验证代理配置生效：向 OS 申请一个空闲端口后立即释放，确保该端口没有任何进程监听，
    /// 以此模拟"代理地址不可达"的场景，请求应当失败（连接被拒绝）。
    /// 这说明 reqwest 确实通过了代理配置，而非绕过直连。
    #[tokio::test]
    async fn test_check_for_update_with_invalid_proxy_fails() {
        // 向 OS 申请一个随机空闲端口，然后立即关闭监听 —— 此时该端口保证无进程占用
        let closed_port = {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            listener.local_addr().unwrap().port()
            // listener 在此处 drop，端口随即释放
        };
        let proxy_url = format!("http://127.0.0.1:{}", closed_port);
        println!("[测试] 使用动态关闭端口: {}", closed_port);

        let result = check_for_update("0.0.0", Some(&proxy_url), None, None).await;

        assert!(
            result.is_err(),
            "使用无效代理时应该返回错误，实际结果: {:?}",
            result
        );
        println!(
            "[预期错误] 代理配置生效，错误信息: {:?}",
            result.unwrap_err()
        );
    }

    /// 对照组：不使用代理时，在网络正常的情况下应能成功获取到版本信息
    #[tokio::test]
    async fn test_check_for_update_no_proxy_succeeds() {
        let result = check_for_update("0.0.0", None, None, None).await;
        assert!(
            result.is_ok(),
            "不使用代理时应该成功，实际错误: {:?}",
            result
        );
        let info = result.unwrap();
        println!(
            "[成功] 当前版本: {}, 最新版本: {}, 有更新: {}",
            info.current_version, info.latest_version, info.has_update
        );
    }
}
