//! 采集调度层。
//!
//! 后端在独立线程里按固定节奏采样，把结果放进 `RwLock` 缓存；
//! Tauri 命令只读缓存，因此前端轮询永远不会阻塞 UI，也不会因为
//! 「一次采样特别慢」而卡住。结束进程等写操作完成后会立刻触发一次重采。

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::thread;
use std::time::Duration;

use crate::error::ProcResult;
use crate::model::{KillResult, ProcessDetail, SystemSnapshot};
use crate::platform::ProcessProvider;

/// 默认刷新间隔（毫秒）。
pub const DEFAULT_INTERVAL_MS: u64 = 1500;
/// 允许的最快刷新间隔——比这更快只会徒增 CPU 占用。
pub const MIN_INTERVAL_MS: u64 = 500;
pub const MAX_INTERVAL_MS: u64 = 60_000;

pub struct ProcessService {
    provider: Arc<dyn ProcessProvider>,
    cache: RwLock<Arc<SystemSnapshot>>,
    interval_ms: AtomicU64,
    /// 串行化「真正去采样」的动作，避免手动刷新与后台线程同时采样。
    gate: Mutex<()>,
    last_error: RwLock<Option<String>>,
    running: AtomicBool,
}

impl ProcessService {
    /// 创建服务并同步完成首帧采样。
    ///
    /// 首帧会采样两次：`sysinfo` 的 CPU 使用率依赖两次采样的差值，
    /// 只采一次的话第一帧所有进程的 CPU 都是 0。
    pub fn new(provider: Arc<dyn ProcessProvider>) -> Self {
        let first = provider.snapshot();
        let interval_ms = AtomicU64::new(DEFAULT_INTERVAL_MS);
        let last_error = RwLock::new(None);

        let service = Self {
            provider,
            cache: RwLock::new(Arc::new(match &first {
                Ok(snapshot) => snapshot.clone(),
                Err(err) => {
                    *last_error.write().unwrap_or_else(|e| e.into_inner()) =
                        Some(err.to_string());
                    empty_snapshot()
                }
            })),
            interval_ms,
            gate: Mutex::new(()),
            last_error,
            running: AtomicBool::new(false),
        };

        thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
        service.refresh_now();
        service
    }

    pub fn provider(&self) -> &Arc<dyn ProcessProvider> {
        &self.provider
    }

    /// 后台采样线程。
    pub fn start_background(self: &Arc<Self>) {
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }
        let service = Arc::clone(self);
        thread::Builder::new()
            .name("procman-sampler".into())
            .spawn(move || {
                loop {
                    // 以 200ms 为粒度睡眠，这样调整间隔后能立刻生效。
                    let mut slept = 0u64;
                    let target = service.interval();
                    while slept < target {
                        let step = 200u64.min(target - slept);
                        thread::sleep(Duration::from_millis(step));
                        slept += step;
                    }
                    service.refresh_now();
                }
            })
            .expect("failed to spawn sampler thread");
    }

    /// 当前缓存的快照（零成本，直接返回 `Arc`）。
    pub fn snapshot(&self) -> Arc<SystemSnapshot> {
        Arc::clone(&self.cache.read().unwrap_or_else(|e| e.into_inner()))
    }

    /// 立即重新采样并写入缓存。
    pub fn refresh_now(&self) -> Arc<SystemSnapshot> {
        let _guard = self.gate.lock().unwrap_or_else(|e| e.into_inner());
        match self.provider.snapshot() {
            Ok(snapshot) => {
                *self.last_error.write().unwrap_or_else(|e| e.into_inner()) = None;
                let snapshot = Arc::new(snapshot);
                *self.cache.write().unwrap_or_else(|e| e.into_inner()) = Arc::clone(&snapshot);
                snapshot
            }
            Err(err) => {
                let message = err.to_string();
                *self.last_error.write().unwrap_or_else(|e| e.into_inner()) = Some(message);
                self.snapshot()
            }
        }
    }

    pub fn interval(&self) -> u64 {
        self.interval_ms.load(Ordering::Relaxed)
    }

    pub fn set_interval(&self, millis: u64) -> u64 {
        let clamped = millis.clamp(MIN_INTERVAL_MS, MAX_INTERVAL_MS);
        self.interval_ms.store(clamped, Ordering::Relaxed);
        clamped
    }

    pub fn last_error(&self) -> Option<String> {
        self.last_error
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    // --- 以下是会改变系统状态的写操作，完成后立刻重采 ---

    pub fn kill(&self, pid: u32, force: bool) -> ProcResult<()> {
        let result = self.provider.kill(pid, force);
        self.refresh_now();
        result
    }

    pub fn kill_tree(&self, pid: u32, force: bool) -> ProcResult<KillResult> {
        let result = self.provider.kill_tree(pid, force);
        self.refresh_now();
        result
    }

    pub fn detail(&self, pid: u32) -> ProcResult<ProcessDetail> {
        self.provider.detail(pid)
    }

    pub fn open_file_location(&self, path: &str) -> ProcResult<()> {
        self.provider.open_file_location(path)
    }

    pub fn icon(&self, exe: &str) -> Option<String> {
        self.provider.icon(exe)
    }

    pub fn focus_window(&self, pid: u32) -> bool {
        self.provider.focus_window(pid)
    }
}

fn empty_snapshot() -> SystemSnapshot {
    SystemSnapshot {
        sequence: 0,
        captured_at: 0,
        elapsed_ms: 0,
        platform: "unknown".into(),
        processes: Vec::new(),
        totals: crate::model::SystemTotals {
            process_count: 0,
            total_memory: 0,
            used_memory: 0,
            total_swap: 0,
            used_swap: 0,
            cpu_count: 0,
            global_cpu_usage: 0.0,
            host_name: "-".into(),
            os_name: "-".into(),
            os_version: "-".into(),
            kernel_version: "-".into(),
            uptime: 0,
            elevated: false,
            listening_port_count: 0,
            connection_count: 0,
        },
        warnings: Vec::new(),
    }
}
