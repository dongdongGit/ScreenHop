use crate::{MonitorInfo, Point, Rect};

/// 计算窗口从当前显示器移动到目标显示器后的新位置
///
/// 使用相对坐标映射：保持窗口在当前显示器上的相对位置比例，
/// 映射到目标显示器的对应位置。
pub fn calculate_new_position(
    window_frame: &Rect,
    current_monitor: &MonitorInfo,
    next_monitor: &MonitorInfo,
) -> (Point, f64, f64) {
    let current_work = &current_monitor.work_area;
    let next_work = &next_monitor.work_area;

    // 计算窗口在当前显示器上的相对位置
    let rel_x = if current_work.width > 0.0 {
        (window_frame.x - current_work.x) / current_work.width
    } else {
        0.0
    };
    let rel_y = if current_work.height > 0.0 {
        (window_frame.y - current_work.y) / current_work.height
    } else {
        0.0
    };

    // 映射到目标显示器
    let mut new_x = next_work.x + (next_work.width * rel_x);
    let mut new_y = next_work.y + (next_work.height * rel_y);

    // 计算窗口在目标显示器上的尺寸（不超过工作区）
    let final_width = window_frame.width.min(next_work.width);
    let final_height = window_frame.height.min(next_work.height);

    // 边界保护：确保窗口不超出工作区
    if new_x < next_work.min_x() {
        new_x = next_work.min_x();
    }
    if new_y < next_work.min_y() {
        new_y = next_work.min_y();
    }
    if new_x + final_width > next_work.max_x() {
        new_x = next_work.max_x() - final_width;
    }
    if new_y + final_height > next_work.max_y() {
        new_y = next_work.max_y() - final_height;
    }

    (Point { x: new_x, y: new_y }, final_width, final_height)
}

/// 根据窗口中心点找到所在的显示器（返回索引）
pub fn find_monitor_for_point(point: Point, monitors: &[MonitorInfo]) -> Option<usize> {
    monitors.iter().position(|m| m.bounds.contains(point))
}

/// 获取下一个显示器的索引（循环）
pub fn next_monitor_index(current: usize, total: usize) -> usize {
    (current + 1) % total
}

/// 判断给定点是否在标题栏区域内
pub fn is_in_title_bar(point: Point, window_frame: &Rect, title_bar_height: f64) -> bool {
    point.x >= window_frame.min_x()
        && point.x <= window_frame.max_x()
        && point.y >= window_frame.min_y()
        && point.y <= window_frame.min_y() + title_bar_height
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_monitor(id: u64, x: f64, y: f64, w: f64, h: f64) -> MonitorInfo {
        MonitorInfo {
            id,
            bounds: Rect::new(x, y, w, h),
            work_area: Rect::new(x, y, w, h),
        }
    }

    #[test]
    fn test_find_monitor_for_point() {
        let monitors = vec![
            make_monitor(1, 0.0, 0.0, 1920.0, 1080.0),
            make_monitor(2, 1920.0, 0.0, 2560.0, 1440.0),
        ];

        assert_eq!(
            find_monitor_for_point(Point { x: 500.0, y: 500.0 }, &monitors),
            Some(0)
        );
        assert_eq!(
            find_monitor_for_point(Point { x: 2000.0, y: 500.0 }, &monitors),
            Some(1)
        );
        assert_eq!(
            find_monitor_for_point(Point { x: -100.0, y: 500.0 }, &monitors),
            None
        );
    }

    #[test]
    fn test_next_monitor_index() {
        assert_eq!(next_monitor_index(0, 2), 1);
        assert_eq!(next_monitor_index(1, 2), 0);
        assert_eq!(next_monitor_index(2, 3), 0);
    }

    #[test]
    fn test_calculate_new_position_centered() {
        let m1 = make_monitor(1, 0.0, 0.0, 1920.0, 1080.0);
        let m2 = make_monitor(2, 1920.0, 0.0, 2560.0, 1440.0);

        // 窗口在 M1 中心
        let window = Rect::new(760.0, 340.0, 400.0, 400.0);
        let (pos, w, h) = calculate_new_position(&window, &m1, &m2);

        // 相对位置应映射到 M2 的相对位置
        assert!(pos.x >= m2.work_area.min_x());
        assert!(pos.y >= m2.work_area.min_y());
        assert!(pos.x + w <= m2.work_area.max_x());
        assert!(pos.y + h <= m2.work_area.max_y());
        assert_eq!(w, 400.0);
        assert_eq!(h, 400.0);
    }

    #[test]
    fn test_calculate_new_position_clamps_size() {
        let m1 = make_monitor(1, 0.0, 0.0, 1920.0, 1080.0);
        let m2 = make_monitor(2, 1920.0, 0.0, 800.0, 600.0);

        // 窗口比目标显示器还大
        let window = Rect::new(0.0, 0.0, 1920.0, 1080.0);
        let (pos, w, h) = calculate_new_position(&window, &m1, &m2);

        assert_eq!(w, 800.0);
        assert_eq!(h, 600.0);
        assert_eq!(pos.x, 1920.0);
        assert_eq!(pos.y, 0.0);
    }

    #[test]
    fn test_is_in_title_bar() {
        let frame = Rect::new(100.0, 100.0, 800.0, 600.0);
        assert!(is_in_title_bar(Point { x: 500.0, y: 110.0 }, &frame, 40.0));
        assert!(!is_in_title_bar(Point { x: 500.0, y: 200.0 }, &frame, 40.0));
        assert!(!is_in_title_bar(Point { x: 50.0, y: 110.0 }, &frame, 40.0));
    }
}
