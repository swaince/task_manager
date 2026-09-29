//! 平台相关能力的「原语层」。
//!
//! `provider.rs` 里只写与平台无关的编排逻辑，所有真正依赖操作系统的能力
//! （窗口枚举、权限判定、图标抽取、文件管理器定位…）都通过本模块暴露。
//!
//! 扩展新平台时，只需新增一个同名的模块并实现同样的函数签名，
//! 无需改动 `provider.rs` 与前端。

// 平台原语模块统一以 `native` 为名对外暴露（文件按平台分开）。
// 注意：模块目录里刻意不使用 `windows.rs` 这个文件名对应的模块名，
// 否则会在本文件内遮蔽同名的 `windows` crate。
#[cfg(windows)]
#[path = "win.rs"]
pub(crate) mod native;

#[cfg(not(windows))]
#[path = "fallback.rs"]
pub(crate) mod native;

mod provider;

pub use provider::GenericProvider;

use crate::error::ProcResult;
use crate::model::{KillResult, ProcessDetail, SystemSnapshot};
use std::sync::Arc;

/// 平台供应商抽象。
///
/// 上层（`service` / `commands`）只依赖这个 trait，因此新增平台时
/// 唯一需要做的事就是再写一个实现，并在 [`create_provider`] 里按 `cfg` 挑选。
pub trait ProcessProvider: Send + Sync {
    /// 平台标识：`windows` / `macos` / `linux`。
    fn platform(&self) -> &'static str;

    /// 当前程序是否以管理员 / root 权限运行。
    fn is_elevated(&self) -> bool;

    /// 采集一次全量快照。
    fn snapshot(&self) -> ProcResult<SystemSnapshot>;

    /// 获取单个进程的详情（含环境变量、祖先进程链）。
    fn detail(&self, pid: u32) -> ProcResult<ProcessDetail>;

    /// 结束单个进程。
    fn kill(&self, pid: u32, force: bool) -> ProcResult<()>;

    /// 结束进程及其整棵子树。
    fn kill_tree(&self, pid: u32, force: bool) -> ProcResult<KillResult>;

    /// 在系统文件管理器中定位该路径。
    fn open_file_location(&self, path: &str) -> ProcResult<()>;

    /// 取得可执行文件的图标（PNG DataURL），不支持时返回 `None`。
    fn icon(&self, exe: &str) -> Option<String>;

    /// 把目标进程的主窗口切到前台。
    fn focus_window(&self, pid: u32) -> bool;
}

/// 依据编译目标挑选实现。当前只区分「Windows」与「非 Windows」，
/// 后续 macOS / Linux 落地时在这里细化即可。
pub fn create_provider() -> Arc<dyn ProcessProvider> {
    Arc::new(GenericProvider::new())
}
