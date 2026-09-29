/**
 * 后端 Tauri 命令的薄封装。
 *
 * 所有跨进程调用都集中在这里，组件层不直接接触 `invoke`，
 * 便于将来把数据源换成 mock（例如纯浏览器预览）。
 */
import { invoke } from '@tauri-apps/api/core'

import type {
  KillResult,
  ProcessDetail,
  RefreshBounds,
  RuntimeInfo,
  SystemSnapshot,
} from '@/types/process'

/** 读取后端缓存的快照（零等待）。 */
export function listProcesses(): Promise<SystemSnapshot> {
  return invoke<SystemSnapshot>('list_processes')
}

/** 强制后端立刻重新采样。 */
export function refreshProcesses(): Promise<SystemSnapshot> {
  return invoke<SystemSnapshot>('refresh_processes')
}

/** 单个进程详情：环境变量、祖先进程链、全部套接字。 */
export function getProcessDetail(pid: number): Promise<ProcessDetail> {
  return invoke<ProcessDetail>('get_process_detail', { pid })
}

/** 结束单个进程；`force` 为 false 时先尝试优雅终止。 */
export function killProcess(pid: number, force: boolean): Promise<number[]> {
  return invoke<number[]>('kill_process', { pid, force })
}

/** 结束进程及其整棵子树。 */
export function killProcessTree(pid: number, force: boolean): Promise<KillResult> {
  return invoke<KillResult>('kill_process_tree', { pid, force })
}

/** 在资源管理器中定位该路径。 */
export function revealInExplorer(path: string): Promise<void> {
  return invoke<void>('reveal_in_explorer', { path })
}

/** 取进程图标（PNG DataURL），后端带缓存。 */
export function getProcessIcon(exe: string): Promise<string | null> {
  return invoke<string | null>('get_process_icon', { exe })
}

/** 把进程主窗口切换到前台。 */
export function focusProcessWindow(pid: number): Promise<boolean> {
  return invoke<boolean>('focus_process_window', { pid })
}

/** 设置后端采样间隔，返回实际生效值。 */
export function setRefreshInterval(millis: number): Promise<number> {
  return invoke<number>('set_refresh_interval', { millis })
}

/** 采样间隔的上下界。 */
export function getRefreshBounds(): Promise<RefreshBounds> {
  return invoke<RefreshBounds>('get_refresh_bounds')
}

/** 运行环境概览：平台、是否提权、版本等。 */
export function getRuntimeInfo(): Promise<RuntimeInfo> {
  return invoke<RuntimeInfo>('get_runtime_info')
}

/** 判断当前是否跑在 Tauri 宿主里（浏览器直开时为 false）。 */
export function isTauriHost(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
}
