/**
 * 与 Rust 后端 `model.rs` 一一对应的类型定义。
 *
 * 后端使用 `#[serde(rename_all = "camelCase")]`，因此这里的字段名可直接对应。
 */

export type Platform = 'windows' | 'macos' | 'linux' | 'unknown'

export interface SocketInfo {
  protocol: 'TCP' | 'UDP'
  family: 'IPv4' | 'IPv6'
  localAddr: string
  localPort: number
  remoteAddr: string | null
  remotePort: number | null
  state: string
  listening: boolean
}

export interface ProcessInfo {
  pid: number
  parentPid: number | null
  name: string
  displayName: string
  exe: string | null
  cmd: string[]
  cmdLine: string
  cwd: string | null
  user: string | null
  status: string
  startTime: number
  runTime: number
  cpuUsage: number
  memory: number
  virtualMemory: number
  threadCount: number
  handleCount: number | null
  ports: number[]
  listeningPorts: number[]
  sockets: SocketInfo[]
  windowTitles: string[]
  accessible: boolean
  children: number[]
}

export interface ProcessBrief {
  pid: number
  name: string
  exe: string | null
}

export interface EnvVar {
  key: string
  value: string
}

export interface ProcessDetail {
  process: ProcessInfo
  environment: EnvVar[]
  ancestors: ProcessBrief[]
  children: ProcessBrief[]
  sockets: SocketInfo[]
}

export interface SystemTotals {
  processCount: number
  totalMemory: number
  usedMemory: number
  totalSwap: number
  usedSwap: number
  cpuCount: number
  globalCpuUsage: number
  hostName: string
  osName: string
  osVersion: string
  kernelVersion: string
  uptime: number
  elevated: boolean
  listeningPortCount: number
  connectionCount: number
}

export interface SystemSnapshot {
  sequence: number
  capturedAt: number
  elapsedMs: number
  platform: Platform
  processes: ProcessInfo[]
  totals: SystemTotals
  warnings: string[]
}

export interface KillFailure {
  pid: number
  name: string
  reason: string
}

export interface KillResult {
  killed: number[]
  failed: KillFailure[]
}

export interface RuntimeInfo {
  platform: Platform
  elevated: boolean
  intervalMs: number
  lastError: string | null
  version: string
}

export interface RefreshBounds {
  min: number
  max: number
}

/** 列表视图可排序的列。 */
export type SortKey =
  | 'name'
  | 'pid'
  | 'cpu'
  | 'memory'
  | 'ports'
  | 'threads'
  | 'startTime'
  | 'user'
  | 'status'

export type SortDirection = 'asc' | 'desc'

/** 主区域视图：平铺列表 / 进程树 / 端口（套接字）列表。 */
export type ViewMode = 'list' | 'tree' | 'ports'

/**
 * 一条端口过滤规则。
 *
 * 由 `portQuery` 的自由文本解析而来，支持三种可选前缀：
 * `tcp:` / `udp:`（协议）、`local:` / `remote:`（匹配哪一端的端口）。
 * 例如 `tcp:local:8000-8100`、`remote:443`、`53`。
 */
export interface PortRule {
  /** 规则原文 token，用于界面回显与删除。 */
  token: string
  /** `null` 表示不限协议。 */
  protocol: 'TCP' | 'UDP' | null
  /** `any` 表示跟随工具栏的「端口作用域」设置。 */
  scope: 'local' | 'remote' | 'any'
  from: number
  to: number
}

/** 端口作用域：匹配本地端口 / 远端端口 / 两者。 */
export type PortScope = 'local' | 'remote' | 'both'

/** 套接字状态筛选。 */
export type SocketStateFilter = 'all' | 'listen' | 'active'

/** 协议筛选。 */
export type SocketProtocolFilter = 'all' | 'TCP' | 'UDP'

/** 端口视图的一行：一条套接字 + 它的宿主进程。 */
export interface PortRow {
  key: string
  pid: number
  name: string
  displayName: string
  exe: string | null
  user: string | null
  socket: SocketInfo
}

/** 端口视图可排序的列。 */
export type PortSortKey = 'port' | 'protocol' | 'state' | 'process'

/** 应用在进程列表上的过滤条件。 */
export interface FilterState {
  query: string
  portQuery: string
  onlyListening: boolean
  onlyWindowed: boolean
}

/** 进程树节点。 */
export interface ProcessNode {
  process: ProcessInfo
  depth: number
  children: ProcessNode[]
  hasChildren: boolean
}
