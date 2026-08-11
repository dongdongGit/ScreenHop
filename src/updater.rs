use anyhow::{Context, Result};
use semver::Version;
use serde::Deserialize;

const GITHUB_API_URL: &str = "https://api.github.com/repos/EcoRoundDev/ScreenHop/releases/latest";
const RELEASES_PAGE_URL: &str = "https://github.com/EcoRoundDev/ScreenHop/releases/latest";

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
///
/// - `mock_version`: 若提供，跳过网络请求，直接伪造「发现此版本」用于本地测试
/// - `mock_download_url`: 配合 mock_version 使用，伪造的下载链接（可指向本地 zip 或真实 URL）
pub async fn check_for_update(
    current_version: &str,
    proxy_url: Option<&str>,
    proxy_username: Option<&str>,
    proxy_password: Option<&str>,
) -> Result<UpdateCheckResult> {
    check_for_update_inner(
        current_version,
        proxy_url,
        proxy_username,
        proxy_password,
        None,
        None,
    )
    .await
}

/// 带 mock 参数的版本，供本地测试在线更新流程使用
pub async fn check_for_update_with_mock(
    current_version: &str,
    proxy_url: Option<&str>,
    proxy_username: Option<&str>,
    proxy_password: Option<&str>,
    mock_version: Option<&str>,
    mock_download_url: Option<&str>,
) -> Result<UpdateCheckResult> {
    check_for_update_inner(
        current_version,
        proxy_url,
        proxy_username,
        proxy_password,
        mock_version,
        mock_download_url,
    )
    .await
}

async fn check_for_update_inner(
    current_version: &str,
    proxy_url: Option<&str>,
    proxy_username: Option<&str>,
    proxy_password: Option<&str>,
    mock_version: Option<&str>,
    mock_download_url: Option<&str>,
) -> Result<UpdateCheckResult> {
    // ── Mock 模式：跳过网络请求，直接返回伪造的更新结果（用于本地测试更新流程）──
    if let Some(mock_ver) = mock_version {
        log::warn!(
            "[mock模式] 使用测试版本: {} (当前: {})",
            mock_ver,
            current_version
        );
        let has_update = mock_ver != current_version;
        return Ok(UpdateCheckResult {
            has_update,
            latest_version: mock_ver.to_string(),
            current_version: current_version.to_string(),
            release_url: RELEASES_PAGE_URL.to_string(),
            download_url: mock_download_url.map(|s| s.to_string()),
            asset_name: mock_download_url
                .map(|u| u.split('/').last().unwrap_or("update.zip").to_string()),
            asset_size: 0,
            error_message: None,
        });
    }

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

/// macOS: 将解压目录中的 .app 替换当前运行的 .app，这里使用后台脚本以确保原进程退出后执行，防止 TCC 权限丢失。
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

    // 去除隔離屬性，否則 macOS 會報錯應用損壞且可能導致重新產生 TCC 問題
    let _ = std::process::Command::new("xattr")
        .arg("-rd")
        .arg("com.apple.quarantine")
        .arg(&new_app)
        .status();

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

    let current_pid = std::process::id();

    // 创建更新脚本
    // 我们必须在主进程退出后再进行替换，否则正在运行中的二进制被覆盖或修改，
    // 会导致系统证书缓存/amfid失效，从而使得 TCC权限（如辅助功能）在下次启动时丢失。
    let script_content = format!(
        r#"#!/bin/bash
# 等待主进程退出
while kill -0 {pid} 2>/dev/null; do
    sleep 0.1
done

# 删除旧应用并移动新应用
rm -rf "{old_app}"
mv "{new_app}" "{old_app}"

# 重新启动新应用
open -a "{old_app}"

# 删除更新脚本自身
rm "$0"
"#,
        pid = current_pid,
        old_app = bundle_path.display(),
        new_app = new_app.display()
    );

    let script_path = std::env::temp_dir().join(format!("screenhop_updater_{}.sh", current_pid));
    std::fs::write(&script_path, script_content).context("写入更新脚本失败")?;

    // 添加可执行权限
    std::process::Command::new("chmod")
        .arg("+x")
        .arg(&script_path)
        .status()
        .context("无法为更新脚本添加执行权限")?;

    log::info!("准备执行后台更新脚本，即将退出当前进程...");

    // 启动后台脚本执行替换与重启（detached）
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "nohup \"{}\" >/dev/null 2>&1 &",
            script_path.display()
        ))
        .spawn()
        .context("启动后台更新脚本失败")?;

    Ok(())
}

/// Windows: 将解压目录中的新版 exe 通过 PowerShell 守护脚本替换当前进程
///
/// 原理：与 macOS 的 bash 守护脚本相同——
///   1. 写一个 .ps1 脚本到 %TEMP%
///   2. 脚本等待当前进程 PID 退出
///   3. 用 Move-Item 替换旧 exe
///   4. 启动新 exe
///   5. 自删除脚本
///   6. 后台启动此脚本后立即返回，让调用方 exit(0)
#[cfg(target_os = "windows")]
pub fn apply_update_windows(extract_dir: &std::path::Path) -> Result<()> {
    // 递归查找新版 exe
    let new_exe = find_exe_in_dir(extract_dir).context("解压目录中未找到 .exe 文件")?;
    log::info!("找到新版程序: {:?}", new_exe);

    // 获取当前 exe 路径（即将被替换的目标）
    let current_exe = std::env::current_exe().context("获取当前程序路径失败")?;
    log::info!("当前程序路径: {:?}", current_exe);

    let current_pid = std::process::id();

    // 构造 PowerShell 守护脚本
    // 等待当前进程退出 → 替换 exe → 重启 → 自删除
    let script_content = format!(
        r#"
# Wait for old process to exit
try {{ Wait-Process -Id {pid} -ErrorAction SilentlyContinue }} catch {{}}
Start-Sleep -Milliseconds 500

# Replace exe (retry up to 5 times in case of brief file lock)
$retries = 5
for ($i = 0; $i -lt $retries; $i++) {{
    try {{
        Move-Item -Path '{new_exe}' -Destination '{old_exe}' -Force
        break
    }} catch {{
        Start-Sleep -Milliseconds 500
    }}
}}

# Launch new version
Start-Process -FilePath '{old_exe}'

# Self-delete this script
Remove-Item -Path $MyInvocation.MyCommand.Path -Force
"#,
        pid = current_pid,
        new_exe = new_exe.display(),
        old_exe = current_exe.display(),
    );

    let script_path = std::env::temp_dir().join(format!("screenhop_updater_{}.ps1", current_pid));

    // 写入 UTF-8 BOM，确保 PowerShell 5.1 在所有 Windows 系统上正确识别编码
    let mut bom_content = vec![0xEF_u8, 0xBB, 0xBF]; // UTF-8 BOM
    bom_content.extend_from_slice(script_content.as_bytes());
    std::fs::write(&script_path, &bom_content).context("写入更新脚本失败")?;
    log::info!("更新脚本已写入: {:?}", script_path);

    // 后台启动 PowerShell 执行脚本（-WindowStyle Hidden 隐藏窗口）
    std::process::Command::new("powershell.exe")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-WindowStyle",
            "Hidden",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            script_path.to_str().unwrap_or_default(),
        ])
        .spawn()
        .context("启动后台更新脚本失败")?;

    log::info!(
        "后台更新脚本已启动（PID={}），等待当前进程退出后自动替换",
        current_pid
    );
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

    /// 验证 Windows 平台下 PowerShell 更新替换及重启脚本的可行性与正确性
    #[tokio::test]
    #[cfg(target_os = "windows")]
    async fn test_update_script_execution() {
        use std::fs;
        use std::process::Command;

        let temp_dir = std::env::temp_dir().join("screenhop_test_update_script");
        let _ = fs::remove_dir_all(&temp_dir);
        fs::create_dir_all(&temp_dir).unwrap();

        // 1. 准备旧版与新版虚拟 exe 文件
        // Windows 系统里内置有 C:\Windows\System32\cmd.exe，我们将其复制作为测试目标
        let old_exe = temp_dir.join("old_dummy.exe");
        let new_exe = temp_dir.join("new_dummy.exe");
        fs::copy("C:\\Windows\\System32\\cmd.exe", &old_exe).unwrap();
        fs::copy("C:\\Windows\\System32\\cmd.exe", &new_exe).unwrap();

        // 2. 启动一个虚拟的"旧进程"，使其运行一段时间以便 PowerShell 脚本等待它退出
        let mut child = Command::new(&old_exe)
            .args(["/c", "start-sleep -s 2"])
            .spawn()
            .unwrap();
        let dummy_pid = child.id();

        // 3. 构建类似于 apply_update_windows 中的 PowerShell 脚本内容
        // 脚本工作：等待进程退出 -> 覆盖文件 -> 自动拉起新版 exe 并写入一个标记成功的文件
        let verify_file = temp_dir.join("success.txt");
        let script_content = format!(
            r#"
# 等待虚拟旧进程退出
try {{ Wait-Process -Id {pid} -ErrorAction SilentlyContinue }} catch {{}}
Start-Sleep -Milliseconds 200

# 覆盖旧文件
$retries = 5
for ($i = 0; $i -lt $retries; $i++) {{
    try {{
        Move-Item -Path '{new_exe}' -Destination '{old_exe}' -Force
        break
    }} catch {{
        Start-Sleep -Milliseconds 200
    }}
}}

# 重新自动启动已更新的程序，命令其生成 success.txt 以供断言
Start-Process -FilePath '{old_exe}' -ArgumentList '/c', 'echo success > "{verify_file}"'

# 脚本自删除
Remove-Item -Path $MyInvocation.MyCommand.Path -Force
"#,
            pid = dummy_pid,
            new_exe = new_exe.display().to_string().replace('\\', "\\\\"),
            old_exe = old_exe.display().to_string().replace('\\', "\\\\"),
            verify_file = verify_file.display().to_string().replace('\\', "\\\\"),
        );

        let script_path = temp_dir.join("test_updater.ps1");
        let mut bom_content = vec![0xEF_u8, 0xBB, 0xBF];
        bom_content.extend_from_slice(script_content.as_bytes());
        fs::write(&script_path, &bom_content).unwrap();

        // 4. 后台静默启动此 PowerShell 脚本
        Command::new("powershell.exe")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-WindowStyle",
                "Hidden",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                script_path.to_str().unwrap(),
            ])
            .spawn()
            .unwrap();

        // 5. 主动关闭该虚拟旧进程，触发 PowerShell 脚本开始工作
        let _ = child.kill();

        // 6. 轮询检测 success.txt 文件是否被自动运行的新程序生成（超时设为 5 秒）
        let start_time = std::time::Instant::now();
        let mut success = false;
        while start_time.elapsed().as_secs() < 5 {
            if verify_file.exists() {
                success = true;
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        // 清理临时文件目录
        let _ = fs::remove_dir_all(&temp_dir);

        // 断言：脚本确实自动拉起了新进程并运行成功
        assert!(success, "PowerShell 更新脚本未能成功拉起被更新的程序！");
    }
}
