use anyhow::Result;
use screenhop_core::config::AppConfig;
use screenhop_core::monitor;
use screenhop_core::Point;

/// 安装鼠标中键钩子，注册事件处理逻辑
pub fn install_hook(config: &AppConfig) -> Result<()> {
    let title_bar_height = config.title_bar_height;

    #[cfg(target_os = "macos")]
    {
        use screenhop_platform::macos::hook::MacMouseHook;
        let mut hook = MacMouseHook::new();
        hook.install_event_tap(move |event| {
            handle_middle_click(event.point, title_bar_height)
        })?;
    }

    #[cfg(target_os = "windows")]
    {
        use screenhop_platform::windows::hook::WinMouseHook;
        let mut hook = WinMouseHook::new();
        hook.install_hook(move |event| {
            handle_middle_click(event.point, title_bar_height)
        })?;
    }

    log::info!("鼠标中键移动引擎已启动");
    Ok(())
}

/// 处理中键点击事件
/// 返回 true 表示事件已消费（窗口已移动），返回 false 表示放行事件
fn handle_middle_click(point: Point, title_bar_height: f64) -> bool {
    use screenhop_platform::{HitTester, MonitorManager, WindowManager};

    // 根据平台创建对应实现
    #[cfg(target_os = "macos")]
    let (wm, monitor_mgr, hit_tester) = {
        use screenhop_platform::macos::{
            hittest::MacHitTester, monitor::MacMonitorManager, window::MacWindowManager,
        };
        let mut ht = MacHitTester::new();
        ht.set_title_bar_height(title_bar_height);
        (MacWindowManager::new(), MacMonitorManager::new(), ht)
    };

    #[cfg(target_os = "windows")]
    let (wm, monitor_mgr, hit_tester) = {
        use screenhop_platform::windows::{
            hittest::WinHitTester, monitor::WinMonitorManager, window::WinWindowManager,
        };
        (WinWindowManager::new(), WinMonitorManager::new(), WinHitTester::new())
    };

    // 1. 获取点击位置的窗口
    let handle = match wm.get_window_at(point) {
        Some(h) => h,
        None => {
            log::debug!("点击位置没有窗口");
            return false;
        }
    };

    // 2. 检查是否点击在交互式标签页上（不移动）
    if hit_tester.is_interactive_tab(&handle, point) {
        log::debug!("点击在交互式标签页上，跳过");
        return false;
    }

    // 3. 获取窗口 frame
    let frame = match wm.get_window_frame(&handle) {
        Some(f) => f,
        None => {
            log::debug!("无法获取窗口 frame");
            return false;
        }
    };

    // 4. 检查是否在标题栏区域内
    #[cfg(target_os = "macos")]
    let in_title_bar = monitor::is_in_title_bar(point, &frame, title_bar_height);

    #[cfg(target_os = "windows")]
    let in_title_bar = hit_tester.is_title_bar_hit(&handle, point);

    if !in_title_bar {
        log::debug!("点击不在标题栏内");
        return false;
    }

    // 5. 获取所有显示器
    let monitors = monitor_mgr.get_monitors();
    if monitors.len() < 2 {
        log::debug!("只有一个显示器，无法移动");
        return false;
    }

    // 6. 找到窗口当前所在的显示器
    let window_center = Point {
        x: frame.mid_x(),
        y: frame.mid_y(),
    };
    let current_idx = match monitor::find_monitor_for_point(window_center, &monitors) {
        Some(idx) => idx,
        None => {
            log::debug!("无法确定窗口所在显示器");
            return false;
        }
    };

    // 7. 计算目标显示器和新位置
    let next_idx = monitor::next_monitor_index(current_idx, monitors.len());
    let (new_pos, new_width, new_height) =
        monitor::calculate_new_position(&frame, &monitors[current_idx], &monitors[next_idx]);

    log::info!(
        "移动窗口: 显示器 {} → {}, 位置 ({:.0},{:.0}) → ({:.0},{:.0})",
        current_idx,
        next_idx,
        frame.x,
        frame.y,
        new_pos.x,
        new_pos.y,
    );

    // 8. Windows: 如果窗口是最大化的，先还原
    #[cfg(target_os = "windows")]
    {
        if wm.is_maximized(&handle) {
            if let Err(e) = wm.restore_window(&handle) {
                log::error!("还原窗口失败: {}", e);
            }
        }
    }

    // 9. 设置新位置和尺寸
    if let Err(e) = wm.set_window_position(&handle, new_pos) {
        log::error!("设置窗口位置失败: {}", e);
        return false;
    }

    // 如果窗口尺寸需要调整（目标显示器更小）
    if (new_width - frame.width).abs() > 1.0 || (new_height - frame.height).abs() > 1.0 {
        if let Err(e) = wm.set_window_size(&handle, new_width, new_height) {
            log::error!("设置窗口尺寸失败: {}", e);
        }
    }

    // 10. 激活窗口
    if let Err(e) = wm.activate_window(&handle) {
        log::error!("激活窗口失败: {}", e);
    }

    true // 事件已消费
}
