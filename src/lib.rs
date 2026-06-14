#[macro_use]
pub mod macros;

pub mod config;
pub mod monitor;
pub mod updater;

// Re-export platform 类型供外部使用
pub use screenhop_platform::{Point, Rect, MonitorInfo};
