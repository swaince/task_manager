//! 非 Windows 平台的占位实现。
//!
//! 目前产品目标是 Windows，但架构上已经把平台能力收敛到这一组函数。
//! 迁移到 macOS / Linux 时，只需在这里按 `#[cfg(target_os = "...")]`
//! 分别给出真实实现（例如 macOS 可用 `CGWindowListCopyWindowInfo`，
//! Linux 可用 X11/Wayland 对应接口）。

use crate::error::{ProcError, ProcResult};

pub fn platform_id() -> &'static str {
    if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "unknown"
    }
}

/// 无法在不支持 `OpenProcess` 语义的平台上判定，统一返回 false。
pub fn is_elevated() -> bool {
    false
}

/// 无法判定目标进程是否可读时，保守地返回 true（不展示「受保护」标记）。
pub fn is_process_accessible(_pid: u32) -> bool {
    true
}

/// 其他平台的窗口枚举待实现。
pub fn window_titles() -> std::collections::HashMap<u32, Vec<String>> {
    std::collections::HashMap::new()
}

pub fn handle_count(_pid: u32) -> Option<u32> {
    None
}

pub fn icon_data_url(_exe: &str) -> Option<String> {
    None
}

pub fn focus_window(_pid: u32) -> bool {
    false
}

pub fn reveal_in_file_manager(_path: &str) -> ProcResult<()> {
    Err(ProcError::Unsupported(
        "当前平台尚未实现「打开文件位置」".into(),
    ))
}
