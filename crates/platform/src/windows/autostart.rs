use anyhow::{anyhow, bail, Context, Result};
use std::{
    env,
    ffi::OsString,
    fs,
    os::windows::{ffi::{OsStrExt, OsStringExt}, process::CommandExt},
    process::Command,
};
use windows::core::{w, PCWSTR, PWSTR};
use windows::Win32::{
    Foundation::{CloseHandle, ERROR_FILE_NOT_FOUND, ERROR_INSUFFICIENT_BUFFER, HANDLE},
    Security::{
        Authorization::ConvertSidToStringSidW, GetSidSubAuthority, GetSidSubAuthorityCount,
        GetTokenInformation, LookupAccountSidW, TokenElevation, TokenElevationType,
        TokenElevationTypeFull, TokenIntegrityLevel, TokenUser, SID_NAME_USE, TOKEN_ELEVATION,
        TOKEN_ELEVATION_TYPE, TOKEN_MANDATORY_LABEL, TOKEN_QUERY, TOKEN_USER,
    },
    System::{
        Registry::{
            RegCloseKey, RegDeleteValueW, RegGetValueW, RegOpenKeyExW, RegSetValueExW, HKEY,
            HKEY_CURRENT_USER, KEY_ALL_ACCESS, REG_SZ, REG_VALUE_TYPE, RRF_RT_REG_SZ,
        },
        SystemInformation::GetLocalTime,
        Threading::{GetCurrentProcess, OpenProcessToken, CREATE_NO_WINDOW},
    },
};

use crate::AutoStart;

const TASK_NAME: &str = "ScreenHop";
const HKEY_RUN: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const HKEY_NAME: PCWSTR = w!("ScreenHop");

pub struct WinAutoStart;

impl Default for WinAutoStart {
    fn default() -> Self {
        Self::new()
    }
}

impl WinAutoStart {
    pub fn new() -> Self {
        Self
    }

    fn exe_path_u16() -> Result<Vec<u16>> {
        let path = std::env::current_exe().context("Failed to get current exe path")?;
        let mut path_u16: Vec<u16> = path.into_os_string().encode_wide().collect();
        path_u16.push(0);
        Ok(path_u16)
    }
}

impl AutoStart for WinAutoStart {
    fn is_enabled(&self) -> bool {
        let is_admin = is_running_as_admin().unwrap_or(false);
        if is_admin {
            exist_scheduled_task(TASK_NAME).unwrap_or(false)
        } else {
            if let Ok(exe_path) = Self::exe_path_u16() {
                reg_is_enable(&exe_path).unwrap_or(false)
            } else {
                false
            }
        }
    }

    fn set_enabled(&self, enabled: bool) -> Result<()> {
        let is_admin = is_running_as_admin().unwrap_or(false);
        let exe_path = Self::exe_path_u16()?;

        if enabled {
            if is_admin {
                let _ = reg_disable(); // To avoid conflicts, if reg is enabled, disable it first.

                let path_str = std::env::current_exe()?.to_string_lossy().to_string();
                create_scheduled_task(TASK_NAME, &path_str)?;
                log::info!("已在计划任务中启用开机自启（管理员模式）");
            } else {
                if exist_scheduled_task(TASK_NAME).unwrap_or(false) {
                    log::warn!("请注意：您曾以管理员身份开启过自启动。为避免冲突，请先以管理员身份运行关闭它，再在普通模式开启。");
                }
                reg_enable(&exe_path)?;
                log::info!("已在注册表中启用开机自启（普通用户模式）");
            }
        } else {
            if is_admin {
                let _ = delete_scheduled_task(TASK_NAME);
                let _ = reg_disable();
                log::info!("已清理计划任务和注册表自启");
            } else {
                reg_disable()?;
                log::info!("已清理注册表自启");
            }
        }
        Ok(())
    }
}

/* ==========================================================================
 * Handle Wrapper
 * ========================================================================== */

#[derive(Debug)]
struct HandleWrapper(HANDLE);
impl Default for HandleWrapper {
    fn default() -> Self {
        Self(HANDLE(std::ptr::null_mut()))
    }
}
impl HandleWrapper {
    fn get_handle_mut(&mut self) -> &mut HANDLE {
        &mut self.0
    }
    fn get_handle(&self) -> HANDLE {
        self.0
    }
}
impl Drop for HandleWrapper {
    fn drop(&mut self) {
        if !self.0 .0.is_null() {
            unsafe {
                let _ = CloseHandle(self.0);
            }
        }
    }
}

/* ==========================================================================
 * Admin & Elevation Check
 * ========================================================================== */

const SECURITY_MANDATORY_HIGH_RID: u32 = 0x00003000;
const SECURITY_MANDATORY_SYSTEM_RID: u32 = 0x00004000;

fn is_running_as_admin() -> Result<bool> {
    let process = unsafe { GetCurrentProcess() };
    get_process_elevation_info(process)
        .map_err(|err| anyhow!("Failed to verify if the program is running as admin, {err}"))
}

fn get_process_elevation_info(process: HANDLE) -> Result<bool> {
    unsafe {
        let mut token = HandleWrapper::default();
        OpenProcessToken(process, TOKEN_QUERY, token.get_handle_mut())?;
        query_token_elevated(token.get_handle())
    }
}

unsafe fn query_token_elevated(token: HANDLE) -> Result<bool> {
    let mut ret_len = 0u32;
    let mut elevation = TOKEN_ELEVATION::default();
    GetTokenInformation(
        token,
        TokenElevation,
        Some(&mut elevation as *mut _ as *mut _),
        std::mem::size_of::<TOKEN_ELEVATION>() as u32,
        &mut ret_len,
    )?;

    let mut elevation_type = TOKEN_ELEVATION_TYPE(0);
    GetTokenInformation(
        token,
        TokenElevationType,
        Some(&mut elevation_type as *mut _ as *mut _),
        std::mem::size_of::<TOKEN_ELEVATION_TYPE>() as u32,
        &mut ret_len,
    )?;

    let mut buf = [0u8; 512];
    GetTokenInformation(
        token,
        TokenIntegrityLevel,
        Some(buf.as_mut_ptr() as *mut _),
        buf.len() as u32,
        &mut ret_len,
    )?;

    let label = &*(buf.as_ptr() as *const TOKEN_MANDATORY_LABEL);
    let sid = label.Label.Sid;
    if sid.0.is_null() {
        bail!("SID is null");
    }
    let sub_auth_count = *GetSidSubAuthorityCount(sid);
    let rid = *GetSidSubAuthority(sid, (sub_auth_count - 1).into());

    Ok(matches!(
        rid,
        SECURITY_MANDATORY_HIGH_RID | SECURITY_MANDATORY_SYSTEM_RID
    ) && elevation.TokenIsElevated != 0
        && elevation_type == TokenElevationTypeFull)
}

/* ==========================================================================
 * Registry Fallback
 * ========================================================================== */

struct RegKey {
    hkey: HKEY,
    name: PCWSTR,
}

impl RegKey {
    fn new_hkcu(subkey: PCWSTR, name: PCWSTR) -> Result<RegKey> {
        let mut hkey = HKEY::default();
        unsafe {
            RegOpenKeyExW(
                HKEY_CURRENT_USER,
                subkey,
                0,
                KEY_ALL_ACCESS,
                &mut hkey as *mut _,
            )
        }
        .ok()
        .map_err(|err| anyhow!("Fail to open reg key, {:?}", err))?;
        Ok(RegKey { hkey, name })
    }

    fn get_value(&self) -> Result<Option<Vec<u16>>> {
        let mut buffer = [0u16; 1024];
        let mut size: u32 = (1024 * std::mem::size_of_val(&buffer[0])) as u32;
        let mut kind: REG_VALUE_TYPE = Default::default();
        let ret = unsafe {
            RegGetValueW(
                self.hkey,
                None,
                self.name,
                RRF_RT_REG_SZ,
                Some(&mut kind),
                Some(buffer.as_mut_ptr() as *mut _),
                Some(&mut size),
            )
        };
        if ret.is_err() {
            if ret == ERROR_FILE_NOT_FOUND {
                return Ok(None);
            }
            bail!(
                "Fail to get reg value, {:?}",
                windows::core::Error::from(ret)
            );
        }
        let len = (size as usize - 1) / 2; // size includes null terminator
        Ok(Some(buffer[..len].to_vec()))
    }

    fn set_value(&self, value: &[u8]) -> Result<()> {
        unsafe { RegSetValueExW(self.hkey, self.name, 0, REG_SZ, Some(value)) }
            .ok()
            .map_err(|err| anyhow!("Fail to write reg value, {:?}", err))?;
        Ok(())
    }

    fn delete_value(&self) -> Result<()> {
        unsafe { RegDeleteValueW(self.hkey, self.name) }
            .ok()
            .map_err(|err| anyhow!("Failed to delete reg value, {:?}", err))?;
        Ok(())
    }
}

impl Drop for RegKey {
    fn drop(&mut self) {
        let _ = unsafe { RegCloseKey(self.hkey) };
    }
}

fn reg_key() -> Result<RegKey> {
    RegKey::new_hkcu(HKEY_RUN, HKEY_NAME)
}

fn reg_is_enable(exe_path: &[u16]) -> Result<bool> {
    let key = reg_key()?;
    let value = match key.get_value()? {
        Some(value) => value,
        None => return Ok(false),
    };
    let exe_path_no_null = if exe_path.last() == Some(&0) {
        &exe_path[..exe_path.len() - 1]
    } else {
        exe_path
    };
    Ok(value == exe_path_no_null)
}

fn reg_enable(exe_path: &[u16]) -> Result<()> {
    let key = reg_key()?;
    let path = unsafe { exe_path.align_to::<u8>().1 };
    key.set_value(path)?;
    Ok(())
}

fn reg_disable() -> Result<()> {
    let key = reg_key()?;
    key.delete_value()?;
    Ok(())
}

/* ==========================================================================
 * Scheduled Task (Admin)
 * ========================================================================== */

fn create_scheduled_task(name: &str, exe_path: &str) -> Result<()> {
    let task_xml_path = create_task_file(name, exe_path)?;
    let output = Command::new("schtasks")
        .creation_flags(CREATE_NO_WINDOW.0)
        .args(["/create", "/tn", name, "/xml", &task_xml_path, "/f"])
        .output()?;
    if !output.status.success() {
        bail!(
            "Fail to create scheduled task, {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let _ = fs::remove_file(&task_xml_path); // Cleanup temp xml
    Ok(())
}

fn delete_scheduled_task(name: &str) -> Result<()> {
    let output = Command::new("schtasks")
        .creation_flags(CREATE_NO_WINDOW.0)
        .args(["/delete", "/tn", name, "/f"])
        .output()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        // It's possible the task doesn't exist, which is fine
        if !stderr.contains("ERROR: The system cannot find the file specified") {
            bail!("Fail to delete scheduled task, {stderr}");
        }
    }
    Ok(())
}

fn exist_scheduled_task(name: &str) -> Result<bool> {
    let output = Command::new("schtasks")
        .creation_flags(CREATE_NO_WINDOW.0)
        .args(["/query", "/tn", name])
        .output()?;
    Ok(output.status.success())
}

fn create_task_file(name: &str, exe_path: &str) -> Result<String> {
    let (author, user_id) = get_author_and_userid()?;
    let current_time = get_current_time();
    let command_path = if exe_path.contains(char::is_whitespace) {
        format!("\"{exe_path}\"")
    } else {
        exe_path.to_string()
    };

    let xml_data = format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Date>{current_time}</Date>
    <Author>{author}</Author>
    <URI>\{name}</URI>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <StartBoundary>{current_time}</StartBoundary>
      <Enabled>true</Enabled>
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      <UserId>{user_id}</UserId>
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>HighestAvailable</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>true</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>true</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>7</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command_path}</Command>
    </Exec>
  </Actions>
</Task>"#
    );
    let xml_path = env::temp_dir().join(format!("{name}-task.xml"));
    let xml_path_str = xml_path.display().to_string();
    fs::write(&xml_path_str, xml_data)?;
    Ok(xml_path_str)
}

fn get_author_and_userid() -> Result<(String, String)> {
    let mut token_handle = HandleWrapper::default();
    unsafe {
        OpenProcessToken(
            GetCurrentProcess(),
            TOKEN_QUERY,
            token_handle.get_handle_mut(),
        )?
    };

    let mut token_info_length = 0;
    if let Err(err) = unsafe {
        GetTokenInformation(
            token_handle.get_handle(),
            TokenUser,
            None,
            0,
            &mut token_info_length,
        )
    } {
        if err.code() != ERROR_INSUFFICIENT_BUFFER.into() {
            return Err(err.into());
        }
    }

    let mut token_user = Vec::<u8>::with_capacity(token_info_length as usize);
    unsafe {
        GetTokenInformation(
            token_handle.get_handle(),
            TokenUser,
            Some(token_user.as_mut_ptr() as *mut _),
            token_info_length,
            &mut token_info_length,
        )?
    };

    let user_sid = unsafe { *(token_user.as_ptr() as *const TOKEN_USER) }
        .User
        .Sid;

    let mut name = Vec::<u16>::with_capacity(256);
    let mut name_len = 256;
    let mut domain = Vec::<u16>::with_capacity(256);
    let mut domain_len = 256;
    let mut sid_name_use = SID_NAME_USE(0);

    unsafe {
        LookupAccountSidW(
            None,
            user_sid,
            PWSTR(name.as_mut_ptr()),
            &mut name_len,
            PWSTR(domain.as_mut_ptr()),
            &mut domain_len,
            &mut sid_name_use,
        )?
    };

    unsafe {
        name.set_len(name_len as usize);
        domain.set_len(domain_len as usize);
    }

    let username = OsString::from_wide(&name).to_string_lossy().into_owned();
    let domainname = OsString::from_wide(&domain).to_string_lossy().into_owned();

    let mut sid_string = PWSTR::null();
    unsafe { ConvertSidToStringSidW(user_sid, &mut sid_string)? };

    let sid_str = OsString::from_wide(unsafe { sid_string.as_wide() })
        .to_string_lossy()
        .into_owned();

    Ok((format!("{domainname}\\{username}"), sid_str))
}

fn get_current_time() -> String {
    let st = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::AutoStart;

    #[test]
    fn test_autostart_toggle() {
        let autostart = WinAutoStart::new();
        // 开启自启
        autostart.set_enabled(true).expect("Failed to enable");
        assert!(autostart.is_enabled(), "Autostart should be enabled");
        // 关闭自启
        autostart.set_enabled(false).expect("Failed to disable");
        assert!(!autostart.is_enabled(), "Autostart should be disabled");
    }
}
