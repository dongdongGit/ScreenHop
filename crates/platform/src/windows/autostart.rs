use anyhow::{Context, Result};
use std::process::Command;

use crate::AutoStart;

/// Windows 自启动管理器（基于 schtasks）
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
}

const TASK_NAME: &str = "ScreenHop";

impl AutoStart for WinAutoStart {
    fn is_enabled(&self) -> bool {
        let output = Command::new("schtasks.exe")
            .args(["/Query", "/TN", TASK_NAME])
            .output();

        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    fn set_enabled(&self, enabled: bool) -> Result<()> {
        if enabled {
            let exe_path = std::env::current_exe()
                .context("获取当前程序路径失败")?;

            let exe_str = exe_path
                .to_str()
                .context("程序路径转换失败")?;

            let args = format!(
                "/Create /TN \"{}\" /TR \"\\\"{}\\\"\" /SC ONLOGON /RL HIGHEST /F",
                TASK_NAME, exe_str
            );

            let output = Command::new("schtasks.exe")
                .args(args.split_whitespace())
                .output()
                .context("执行 schtasks 失败")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                anyhow::bail!("创建计划任务失败: {}", stderr);
            }

            log::info!("已创建开机自启动计划任务");
        } else {
            let output = Command::new("schtasks.exe")
                .args(["/Delete", "/TN", TASK_NAME, "/F"])
                .output()
                .context("执行 schtasks 失败")?;

            if !output.status.success() {
                log::warn!("删除计划任务失败（可能不存在）");
            } else {
                log::info!("已删除开机自启动计划任务");
            }
        }

        Ok(())
    }
}
