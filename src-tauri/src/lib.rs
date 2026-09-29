//! 进程管理器后端入口。
//!
//! 分层：
//! ```text
//! commands  ←  Tauri IPC 边界（只做参数/返回值的搬运）
//!    ↓
//! service   ←  采样调度：后台线程 + 缓存 + 写操作后立即重采
//!    ↓
//! platform  ←  ProcessProvider trait；native 子模块收敛平台差异
//!    ↓
//! model / error  ←  跨语言传输的数据结构与错误
//! ```

mod commands;
mod error;
mod model;
mod platform;
mod service;

use std::sync::Arc;

use commands::AppState;
use service::ProcessService;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let provider = platform::create_provider();
    let service = Arc::new(ProcessService::new(provider));
    service.start_background();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(AppState { service })
        .invoke_handler(tauri::generate_handler![
            commands::list_processes,
            commands::refresh_processes,
            commands::get_process_detail,
            commands::kill_process,
            commands::kill_process_tree,
            commands::reveal_in_explorer,
            commands::get_process_icon,
            commands::focus_process_window,
            commands::set_refresh_interval,
            commands::get_refresh_bounds,
            commands::get_runtime_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
