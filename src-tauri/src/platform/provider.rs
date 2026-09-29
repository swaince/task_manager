//! 平台无关的进程信息编排层。
//!
//! 数据来源分成三块，在这里合并成同一个快照：
//! * **进程属性** —— `sysinfo`（Windows / macOS / Linux 通吃）；
//! * **网络套接字** —— `netstat2`（同样是跨平台实现，Windows 走 `GetExtendedTcpTable`）；
//! * **平台原语** —— 窗口标题、权限、句柄数等，见 `platform::native`。
//!
//! 渲染/过滤/排序全部放在前端，后端只负责「如实、尽快地给出数据」。

use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use netstat2::{get_sockets_info, AddressFamilyFlags, ProtocolFlags, ProtocolSocketInfo, TcpState};
use sysinfo::{
    Pid, Process, ProcessRefreshKind, ProcessesToUpdate, Signal, System, UpdateKind, Users,
};

use crate::error::{ProcError, ProcResult};
use crate::model::{
    EnvVar, KillFailure, KillResult, ProcessBrief, ProcessDetail, ProcessDto, SocketDto,
    SystemSnapshot, SystemTotals,
};
use crate::platform::{native, ProcessProvider};

/// 受保护的系统进程，禁止结束。
const PROTECTED_PIDS: [u32; 2] = [0, 4];

pub struct GenericProvider {
    system: Mutex<System>,
    users: Users,
    sequence: AtomicU64,
    elevated: bool,
}

impl GenericProvider {
    pub fn new() -> Self {
        let mut system = System::new();
        // 预热一次，保证第一帧就有 CPU / 内存数据。
        system.refresh_memory();
        system.refresh_cpu_usage();
        Self {
            system: Mutex::new(system),
            users: Users::new_with_refreshed_list(),
            sequence: AtomicU64::new(0),
            elevated: native::is_elevated(),
        }
    }

    fn lock_system(&self) -> std::sync::MutexGuard<'_, System> {
        self.system.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// 轻量刷新：内存、CPU、线程数每轮都取；
    /// 命令行 / 可执行路径 / 工作目录 / 用户等「基本不变」的字段只在首次读取
    /// （`OnlyIfNotSet`），避免每轮都去读一遍所有进程的 PEB。
    fn light_refresh(system: &mut System) {
        system.refresh_memory();
        system.refresh_cpu_usage();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .with_memory()
                .with_cpu()
                .with_tasks()
                .with_exe(UpdateKind::OnlyIfNotSet)
                .with_cmd(UpdateKind::OnlyIfNotSet)
                .with_cwd(UpdateKind::OnlyIfNotSet)
                .with_user(UpdateKind::OnlyIfNotSet),
        );
    }

    fn collect_context(system: &System) -> BuildContext {
        let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
        for (pid, process) in system.processes() {
            if let Some(parent) = process.parent() {
                children
                    .entry(parent.as_u32())
                    .or_default()
                    .push(pid.as_u32());
            }
        }
        for list in children.values_mut() {
            list.sort_unstable();
        }

        let mut warnings = Vec::new();
        let sockets = match collect_sockets() {
            Ok(map) => map,
            Err(err) => {
                warnings.push(err.to_string());
                HashMap::new()
            }
        };

        BuildContext {
            children,
            sockets,
            windows: native::window_titles(),
            warnings,
        }
    }

    fn build_detail(&self, system: &System, pid: u32) -> ProcResult<ProcessDetail> {
        let key = Pid::from_u32(pid);
        let process = system
            .process(key)
            .ok_or(ProcError::ProcessNotFound(pid))?;

        let context = Self::collect_context(system);
        let dto = build_dto(pid, process, &context, &self.users);

        let environment = process
            .environ()
            .iter()
            .map(|entry| {
                let raw = entry.to_string_lossy();
                let (key, value) = match raw.split_once('=') {
                    Some((k, v)) => (k.to_string(), v.to_string()),
                    None => (raw.to_string(), String::new()),
                };
                EnvVar { key, value }
            })
            .collect();

        let mut ancestors = Vec::new();
        let mut cursor = process.parent();
        let mut guard = 0;
        while let Some(parent) = cursor {
            if guard > 64 {
                break;
            }
            guard += 1;
            match system.process(parent) {
                Some(p) => {
                    ancestors.push(brief(parent.as_u32(), p));
                    cursor = p.parent();
                }
                None => break,
            }
        }

        let children = context
            .children
            .get(&pid)
            .map(|ids| {
                ids.iter()
                    .filter_map(|child| system.process(Pid::from_u32(*child)))
                    .map(|p| brief(p.pid().as_u32(), p))
                    .collect()
            })
            .unwrap_or_default();

        Ok(ProcessDetail {
            sockets: dto.sockets.clone(),
            process: dto,
            environment,
            ancestors,
            children,
        })
    }

    /// 收集 `pid` 自身 + 全部后代的 pid，按「深度优先、由深到浅」排序。
    fn descendants(system: &System, pid: u32) -> Vec<u32> {
        let mut depth: HashMap<u32, u32> = HashMap::new();
        let mut stack = vec![(pid, 0u32)];
        depth.insert(pid, 0);
        while let Some((current, level)) = stack.pop() {
            for (child_pid, child) in system.processes() {
                if child.parent().map(|p| p.as_u32()) == Some(current) {
                    let child = child_pid.as_u32();
                    if depth.insert(child, level + 1).is_none() {
                        stack.push((child, level + 1));
                    }
                }
            }
        }
        let mut entries: Vec<(u32, u32)> = depth.into_iter().collect();
        // 深度大的先杀，避免父进程先死导致子进程被系统重新挂到别处。
        entries.sort_by(|a, b| b.1.cmp(&a.1).then(b.0.cmp(&a.0)));
        entries.into_iter().map(|(pid, _)| pid).collect()
    }
}

impl Default for GenericProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessProvider for GenericProvider {
    fn platform(&self) -> &'static str {
        native::platform_id()
    }

    fn is_elevated(&self) -> bool {
        self.elevated
    }

    fn snapshot(&self) -> ProcResult<SystemSnapshot> {
        let started = Instant::now();
        let mut system = self.lock_system();
        Self::light_refresh(&mut system);

        let context = Self::collect_context(&system);

        let mut processes: Vec<ProcessDto> = system
            .processes()
            .iter()
            .map(|(pid, process)| build_dto(pid.as_u32(), process, &context, &self.users))
            .collect();
        processes.sort_by(|a, b| a.pid.cmp(&b.pid));

        let mut listening_ports: HashSet<u16> = HashSet::new();
        let mut connection_count = 0usize;
        for sockets in context.sockets.values() {
            for socket in sockets {
                if socket.listening {
                    listening_ports.insert(socket.local_port);
                } else if socket.protocol == "TCP" {
                    connection_count += 1;
                }
            }
        }

        let totals = SystemTotals {
            process_count: processes.len(),
            total_memory: system.total_memory(),
            used_memory: system.used_memory(),
            total_swap: system.total_swap(),
            used_swap: system.used_swap(),
            cpu_count: system.cpus().len(),
            global_cpu_usage: system.global_cpu_usage(),
            host_name: System::host_name().unwrap_or_else(|| "-".into()),
            os_name: System::name().unwrap_or_else(|| "-".into()),
            os_version: System::os_version().unwrap_or_else(|| "-".into()),
            kernel_version: System::kernel_version().unwrap_or_else(|| "-".into()),
            uptime: System::uptime(),
            elevated: self.elevated,
            listening_port_count: listening_ports.len(),
            connection_count,
        };

        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(SystemSnapshot {
            sequence,
            captured_at: now_millis(),
            elapsed_ms: started.elapsed().as_millis() as u64,
            platform: native::platform_id().to_string(),
            processes,
            totals,
            warnings: context.warnings,
        })
    }

    fn detail(&self, pid: u32) -> ProcResult<ProcessDetail> {
        let mut system = self.lock_system();
        let key = Pid::from_u32(pid);
        // 详情页需要环境变量等「贵」字段，这里只针对单个进程做全量刷新。
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[key]),
            false,
            ProcessRefreshKind::everything(),
        );
        self.build_detail(&system, pid)
    }

    fn kill(&self, pid: u32, force: bool) -> ProcResult<()> {
        let mut system = self.lock_system();
        let key = Pid::from_u32(pid);
        system.refresh_processes(ProcessesToUpdate::Some(&[key]), false);

        let process = system
            .process(key)
            .ok_or(ProcError::ProcessNotFound(pid))?;
        let name = process.name().to_string_lossy().to_string();
        terminate(pid, &name, process, force)
    }

    fn kill_tree(&self, pid: u32, force: bool) -> ProcResult<KillResult> {
        let mut system = self.lock_system();
        system.refresh_processes(ProcessesToUpdate::All, true);

        let targets = Self::descendants(&system, pid);
        let mut killed = Vec::new();
        let mut failed = Vec::new();

        for target in targets {
            let key = Pid::from_u32(target);
            let Some(process) = system.process(key) else {
                continue;
            };
            let name = process.name().to_string_lossy().to_string();
            match terminate(target, &name, process, force) {
                Ok(()) => killed.push(target),
                Err(err) => failed.push(KillFailure {
                    pid: target,
                    name,
                    reason: err.to_string(),
                }),
            }
        }

        if killed.is_empty() && !failed.is_empty() {
            return Err(ProcError::AccessDenied(format!(
                "没有任何进程被结束（{} 个被拒绝）",
                failed.len()
            )));
        }
        Ok(KillResult { killed, failed })
    }

    fn open_file_location(&self, path: &str) -> ProcResult<()> {
        native::reveal_in_file_manager(path)
    }

    fn icon(&self, exe: &str) -> Option<String> {
        native::icon_data_url(exe)
    }

    fn focus_window(&self, pid: u32) -> bool {
        native::focus_window(pid)
    }
}

// ---------------------------------------------------------------------------
// 快照构建
// ---------------------------------------------------------------------------

struct BuildContext {
    children: HashMap<u32, Vec<u32>>,
    sockets: HashMap<u32, Vec<SocketDto>>,
    windows: HashMap<u32, Vec<String>>,
    warnings: Vec<String>,
}

fn build_dto(pid: u32, process: &Process, context: &BuildContext, users: &Users) -> ProcessDto {
    let name = process.name().to_string_lossy().to_string();
    let cmd: Vec<String> = process
        .cmd()
        .iter()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect();
    let sockets = context.sockets.get(&pid).cloned().unwrap_or_default();

    let mut ports: Vec<u16> = sockets.iter().map(|s| s.local_port).collect();
    ports.sort_unstable();
    ports.dedup();
    let mut listening: Vec<u16> = sockets
        .iter()
        .filter(|s| s.listening)
        .map(|s| s.local_port)
        .collect();
    listening.sort_unstable();
    listening.dedup();

    let user = process
        .user_id()
        .and_then(|uid| users.get_user_by_id(uid))
        .map(|user| user.name().to_string());

    ProcessDto {
        pid,
        parent_pid: process.parent().map(|p| p.as_u32()),
        display_name: display_name(&name),
        name,
        exe: process.exe().map(|p| p.to_string_lossy().to_string()),
        cmd_line: join_command_line(&cmd),
        cmd,
        cwd: process.cwd().map(|p| p.to_string_lossy().to_string()),
        user,
        status: format!("{:?}", process.status()),
        start_time: process.start_time(),
        run_time: process.run_time(),
        cpu_usage: process.cpu_usage(),
        memory: process.memory(),
        virtual_memory: process.virtual_memory(),
        thread_count: process.tasks().map(|t| t.len()).unwrap_or(0),
        handle_count: native::handle_count(pid),
        ports,
        listening_ports: listening,
        sockets,
        window_titles: context.windows.get(&pid).cloned().unwrap_or_default(),
        // 双重判定：系统层面能打开该进程，且确实读到了命令行或路径。
        // 任何一条不满足都视为「受保护 / 信息不可读」，前端会打上标记。
        accessible: native::is_process_accessible(pid)
            && (process.exe().is_some() || !process.cmd().is_empty()),
        children: context.children.get(&pid).cloned().unwrap_or_default(),
    }
}

fn brief(pid: u32, process: &Process) -> ProcessBrief {
    ProcessBrief {
        pid,
        name: process.name().to_string_lossy().to_string(),
        exe: process.exe().map(|p| p.to_string_lossy().to_string()),
    }
}

fn display_name(name: &str) -> String {
    name.strip_suffix(".exe")
        .or_else(|| name.strip_suffix(".EXE"))
        .unwrap_or(name)
        .to_string()
}

/// Windows 风格地拼接命令行：含空格的参数用双引号包裹。
fn join_command_line(args: &[String]) -> String {
    args.iter()
        .map(|arg| {
            if arg.is_empty() {
                "\"\"".to_string()
            } else if arg.contains(' ') || arg.contains('\t') {
                format!("\"{arg}\"")
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// 套接字
// ---------------------------------------------------------------------------

fn collect_sockets() -> ProcResult<HashMap<u32, Vec<SocketDto>>> {
    let families = AddressFamilyFlags::IPV4 | AddressFamilyFlags::IPV6;
    let protocols = ProtocolFlags::TCP | ProtocolFlags::UDP;
    let infos =
        get_sockets_info(families, protocols).map_err(|e| ProcError::SocketTable(e.to_string()))?;

    let mut map: HashMap<u32, Vec<SocketDto>> = HashMap::new();
    for info in infos {
        if info.associated_pids.is_empty() {
            continue;
        }
        let socket = match &info.protocol_socket_info {
            ProtocolSocketInfo::Tcp(tcp) => SocketDto {
                protocol: "TCP".into(),
                family: family_of(&tcp.local_addr),
                local_addr: normalize_addr(&tcp.local_addr),
                local_port: tcp.local_port,
                remote_addr: Some(normalize_addr(&tcp.remote_addr)),
                remote_port: Some(tcp.remote_port),
                state: tcp.state.to_string(),
                listening: tcp.state == TcpState::Listen,
            },
            ProtocolSocketInfo::Udp(udp) => SocketDto {
                protocol: "UDP".into(),
                family: family_of(&udp.local_addr),
                local_addr: normalize_addr(&udp.local_addr),
                local_port: udp.local_port,
                remote_addr: None,
                remote_port: None,
                state: "BOUND".into(),
                // UDP 没有 LISTEN 状态，绑定了端口就视为在监听的候选。
                listening: true,
            },
        };
        for pid in &info.associated_pids {
            map.entry(*pid).or_default().push(socket.clone());
        }
    }

    for sockets in map.values_mut() {
        sockets.sort_by(|a, b| {
            (&a.protocol, a.local_port, a.remote_port, &a.state).cmp(&(
                &b.protocol,
                b.local_port,
                b.remote_port,
                &b.state,
            ))
        });
        sockets.dedup_by(|a, b| {
            a.protocol == b.protocol
                && a.local_port == b.local_port
                && a.local_addr == b.local_addr
                && a.remote_port == b.remote_port
                && a.state == b.state
        });
    }
    Ok(map)
}

fn family_of(addr: &IpAddr) -> String {
    match addr {
        IpAddr::V4(_) => "IPv4".into(),
        IpAddr::V6(_) => "IPv6".into(),
    }
}

/// 把 `::ffff:127.0.0.1` 这类 IPv4-mapped 地址还原成 `127.0.0.1`，便于阅读与过滤。
fn normalize_addr(addr: &IpAddr) -> String {
    match addr {
        IpAddr::V4(v4) => v4.to_string(),
        IpAddr::V6(v6) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => v6.to_string(),
        },
    }
}

// ---------------------------------------------------------------------------
// 结束进程
// ---------------------------------------------------------------------------

fn terminate(pid: u32, name: &str, process: &Process, force: bool) -> ProcResult<()> {
    if PROTECTED_PIDS.contains(&pid) {
        return Err(ProcError::Unsupported(format!(
            "系统关键进程 {name}({pid}) 不允许结束"
        )));
    }
    if pid == std::process::id() {
        return Err(ProcError::Unsupported(
            "不能结束进程管理器自身".to_string(),
        ));
    }

    let signal = if force { Signal::Kill } else { Signal::Term };
    if let Some(true) = process.kill_with(signal) {
        return Ok(());
    }
    // 平台不支持优雅终止（例如 Windows 没有 SIGTERM）时退化为强制结束。
    if process.kill() {
        return Ok(());
    }
    Err(ProcError::AccessDenied(format!(
        "{name}({pid}) 拒绝结束，可能需要管理员权限"
    )))
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_contains_current_process() {
        let provider = GenericProvider::new();
        let snapshot = provider.snapshot().expect("采集快照失败");

        assert!(!snapshot.processes.is_empty(), "进程列表不应为空");
        assert_eq!(snapshot.totals.process_count, snapshot.processes.len());
        assert!(snapshot.totals.total_memory > 0, "应读到物理内存总量");
        assert!(snapshot.platform == "windows" || snapshot.platform == "macos" || snapshot.platform == "linux");

        let me = std::process::id();
        let current = snapshot
            .processes
            .iter()
            .find(|p| p.pid == me)
            .expect("当前测试进程应出现在快照中");
        assert!(!current.name.is_empty());
        assert!(current.memory > 0, "当前进程内存占用应大于 0");
        // 注意：Windows 上 virtual_memory 与 memory 的口径不同（私有提交量 vs 工作集），
        // 两者没有必然的大小关系，因此这里不做比较。
    }

    #[test]
    fn listening_socket_is_attributed_to_owner() {
        // 打开一个随机端口，验证「端口 → 进程」的映射链路真的通了。
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("绑定端口失败");
        let port = listener.local_addr().expect("读取本地地址失败").port();

        let sockets = collect_sockets().expect("读取套接字表失败");
        let mine = sockets.get(&std::process::id()).cloned().unwrap_or_default();
        assert!(
            mine.iter().any(|s| s.local_port == port && s.listening),
            "应能识别本进程在 {port} 上的监听套接字，实际为 {mine:?}"
        );
    }

    #[test]
    fn detail_exposes_command_line() {
        let provider = GenericProvider::new();
        let detail = provider.detail(std::process::id()).expect("读取详情失败");

        assert_eq!(detail.process.pid, std::process::id());
        assert!(
            !detail.process.cmd.is_empty() || !detail.process.cmd_line.is_empty(),
            "测试进程应能读到命令行"
        );
        assert!(
            !detail.environment.is_empty(),
            "测试进程应能读到环境变量"
        );
    }

    #[test]
    fn top_level_processes_have_no_parent_in_list() {
        let provider = GenericProvider::new();
        let snapshot = provider.snapshot().expect("采集快照失败");
        let ids: HashSet<u32> = snapshot.processes.iter().map(|p| p.pid).collect();

        // 每个进程的 pid 唯一。
        assert_eq!(ids.len(), snapshot.processes.len(), "PID 不应重复");

        // 子进程列表与实际父子关系自洽：出现过的子 pid 必然也有 parent 指向其父。
        for parent in &snapshot.processes {
            for child in &parent.children {
                let child_process = snapshot
                    .processes
                    .iter()
                    .find(|p| p.pid == *child)
                    .expect("子进程应在列表中");
                assert_eq!(
                    child_process.parent_pid,
                    Some(parent.pid),
                    "父子关系不一致：{child} 的 parent 应为 {}",
                    parent.pid
                );
            }
        }
    }

    #[test]
    fn display_name_strips_exe_suffix() {
        assert_eq!(display_name("chrome.exe"), "chrome");
        assert_eq!(display_name("NOTEPAD.EXE"), "NOTEPAD");
        assert_eq!(display_name("systemd"), "systemd");
    }

    #[test]
    fn command_line_quotes_arguments_with_spaces() {
        let args = vec![
            "C:\\Program Files\\app.exe".to_string(),
            "--flag".to_string(),
            "hello world".to_string(),
            String::new(),
        ];
        assert_eq!(
            join_command_line(&args),
            "\"C:\\Program Files\\app.exe\" --flag \"hello world\" \"\""
        );
    }

    #[test]
    fn ipv4_mapped_addresses_are_normalized() {
        let mapped: IpAddr = "::ffff:127.0.0.1".parse().unwrap();
        assert_eq!(normalize_addr(&mapped), "127.0.0.1");
        assert_eq!(family_of(&mapped), "IPv6");

        let plain: IpAddr = "0.0.0.0".parse().unwrap();
        assert_eq!(normalize_addr(&plain), "0.0.0.0");
        assert_eq!(family_of(&plain), "IPv4");
    }

    #[test]
    fn kill_refuses_critical_and_self() {
        let provider = GenericProvider::new();
        assert!(provider.kill(4, true).is_err(), "不应允许结束 PID 4");
        assert!(
            provider.kill(std::process::id(), true).is_err(),
            "不应允许结束自身"
        );
    }
}
