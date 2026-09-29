//! Tauri 命令层：前端的唯一入口。
//!
//! 命名与返回结构都尽量贴近前端语义，前端只做「拿数据 → 渲染」。

use std::sync::Arc;

use tauri::State;

use crate::error::ProcResult;
use crate::model::{KillResult, ProcessDetail, SystemSnapshot};
use crate::service::{ProcessService, MAX_INTERVAL_MS, MIN_INTERVAL_MS};

pub struct AppState {
    pub service: Arc<ProcessService>,
}

/// 读取当前缓存快照。
#[tauri::command]
pub fn list_processes(state: State<'_, AppState>) -> SystemSnapshot {
    (*state.service.snapshot()).clone()
}

/// 立即重新采样后返回。
#[tauri::command]
pub fn refresh_processes(state: State<'_, AppState>) -> SystemSnapshot {
    (*state.service.refresh_now()).clone()
}

/// 单个进程的完整详情。
#[tauri::command]
pub fn get_process_detail(state: State<'_, AppState>, pid: u32) -> ProcResult<ProcessDetail> {
    state.service.detail(pid)
}

/// 结束单个进程。`force = false` 时优先尝试优雅终止。
#[tauri::command]
pub fn kill_process(state: State<'_, AppState>, pid: u32, force: bool) -> ProcResult<Vec<u32>> {
    state.service.kill(pid, force)?;
    Ok(vec![pid])
}

/// 结束进程树。
#[tauri::command]
pub fn kill_process_tree(
    state: State<'_, AppState>,
    pid: u32,
    force: bool,
) -> ProcResult<KillResult> {
    state.service.kill_tree(pid, force)
}

/// 在资源管理器中定位该可执行文件。
#[tauri::command]
pub fn reveal_in_explorer(state: State<'_, AppState>, path: String) -> ProcResult<()> {
    state.service.open_file_location(&path)
}

/// 取进程图标（PNG DataURL），带后端缓存。
#[tauri::command]
pub fn get_process_icon(state: State<'_, AppState>, exe: String) -> Option<String> {
    state.service.icon(&exe)
}

/// 把进程主窗口切到前台。
#[tauri::command]
pub fn focus_process_window(state: State<'_, AppState>, pid: u32) -> bool {
    state.service.focus_window(pid)
}

/// 调整后端采样间隔，返回实际生效值（毫秒）。
#[tauri::command]
pub fn set_refresh_interval(state: State<'_, AppState>, millis: u64) -> u64 {
    state.service.set_interval(millis)
}

/// 采样间隔的取值边界，前端用来渲染下拉选项。
#[tauri::command]
pub fn get_refresh_bounds() -> serde_json::Value {
    serde_json::json!({ "min": MIN_INTERVAL_MS, "max": MAX_INTERVAL_MS })
}

/// 运行环境概览，用于页脚与「以管理员身份运行」提示。
#[tauri::command]
pub fn get_runtime_info(state: State<'_, AppState>) -> serde_json::Value {
    let provider = state.service.provider();
    serde_json::json!({
        "platform": provider.platform(),
        "elevated": provider.is_elevated(),
        "intervalMs": state.service.interval(),
        "lastError": state.service.last_error(),
        "version": env!("CARGO_PKG_VERSION"),
    })
}
