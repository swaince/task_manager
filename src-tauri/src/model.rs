//! 前后端之间传输的数据结构（DTO）。
//!
//! 全部使用 `camelCase` 序列化，便于 Vue 侧直接消费。
//! 这些结构是「平台无关」的抽象：任何平台的 provider 都产出同一套结构，
//! 平台独有的字段以 `Option` 暴露（例如 Windows 的窗口标题、句柄数）。

use serde::{Deserialize, Serialize};

/// 单个网络套接字（一条 TCP 连接 / 一个 UDP 绑定）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SocketDto {
    /// `TCP` | `UDP`
    pub protocol: String,
    /// `IPv4` | `IPv6`
    pub family: String,
    pub local_addr: String,
    pub local_port: u16,
    pub remote_addr: Option<String>,
    pub remote_port: Option<u16>,
    /// 例如 `LISTEN` / `ESTABLISHED`；UDP 恒为 `BOUND`。
    pub state: String,
    /// 是否为「监听中」的端口（TCP LISTEN 或 UDP 绑定）。
    pub listening: bool,
}

/// 单个进程的完整视图。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessDto {
    pub pid: u32,
    pub parent_pid: Option<u32>,
    /// 原始进程名（Windows 上通常带 `.exe`）。
    pub name: String,
    /// 去扩展名后的展示名。
    pub display_name: String,
    pub exe: Option<String>,
    pub cmd: Vec<String>,
    /// 空格拼接后的命令行（已做必要的引号包裹）。
    pub cmd_line: String,
    pub cwd: Option<String>,
    pub user: Option<String>,
    /// `Run` / `Sleep` / `Zombie` 等。
    pub status: String,
    /// 进程启动时间（Unix 秒）。
    pub start_time: u64,
    /// 已运行秒数。
    pub run_time: u64,
    /// 占单核百分比（可能 > 100）。
    pub cpu_usage: f32,
    /// 常驻内存字节。
    pub memory: u64,
    /// 虚拟内存字节。
    pub virtual_memory: u64,
    pub thread_count: usize,
    /// Windows 专属：内核句柄数。
    pub handle_count: Option<u32>,
    /// 该进程占用的全部本地端口（去重、升序）。
    pub ports: Vec<u16>,
    /// 其中处于监听状态的端口（去重、升序）。
    pub listening_ports: Vec<u16>,
    pub sockets: Vec<SocketDto>,
    /// Windows 专属：可见窗口标题。
    pub window_titles: Vec<String>,
    /// 是否成功读取到受保护信息（cmd/exe）。受保护或提权进程为 false。
    pub accessible: bool,
    /// 直接子进程 pid 列表。
    pub children: Vec<u32>,
}

/// 进程概要，用于父链 / 子进程列表。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessBrief {
    pub pid: u32,
    pub name: String,
    pub exe: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EnvVar {
    pub key: String,
    pub value: String,
}

/// 进程详情（右侧抽屉数据源）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessDetail {
    pub process: ProcessDto,
    /// 环境变量（Windows 上需要足够权限）。
    pub environment: Vec<EnvVar>,
    /// 由近及远的祖先进程链。
    pub ancestors: Vec<ProcessBrief>,
    pub children: Vec<ProcessBrief>,
    /// 该进程的全部套接字（含未归属端口表的补充信息）。
    pub sockets: Vec<SocketDto>,
}

/// 全局统计信息。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemTotals {
    pub process_count: usize,
    pub total_memory: u64,
    pub used_memory: u64,
    pub total_swap: u64,
    pub used_swap: u64,
    pub cpu_count: usize,
    pub global_cpu_usage: f32,
    pub host_name: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub uptime: u64,
    /// 本程序是否以管理员权限运行。
    pub elevated: bool,
    pub listening_port_count: usize,
    pub connection_count: usize,
}

/// 一次完整采样的结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemSnapshot {
    /// 单调递增的采样序号，前端可据此判断数据新鲜度。
    pub sequence: u64,
    /// 采样完成时间（Unix 毫秒）。
    pub captured_at: u64,
    /// 本次采样耗时（毫秒）。
    pub elapsed_ms: u64,
    /// `windows` | `macos` | `linux` | `unknown`
    pub platform: String,
    pub processes: Vec<ProcessDto>,
    pub totals: SystemTotals,
    /// 采样过程中的非致命告警（例如套接字表读取被拒绝）。
    pub warnings: Vec<String>,
}

/// 结束进程的结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillResult {
    pub killed: Vec<u32>,
    pub failed: Vec<KillFailure>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KillFailure {
    pub pid: u32,
    pub name: String,
    pub reason: String,
}
