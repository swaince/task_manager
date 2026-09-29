/**
 * 进程数据的唯一状态源（轻量自研 store，无需引入 Pinia）。
 *
 * 职责：
 * 1. 按用户设定的间隔轮询后端缓存快照；
 * 2. 组合「名称 / 端口 / 仅监听 / 仅有窗口」四类过滤器；
 * 3. 排序、进程树构建，以及端口视图（套接字）的数据与筛选；
 * 4. 懒加载图标与进程详情，避免一次传输数百个 DataURL。
 */
import { computed, reactive, ref } from 'vue'
import { toast } from 'vue-sonner'

import * as api from '@/api/process'
import {
  effectiveScope,
  parsePortQuery,
  ruleToToken,
  socketMatchesAnyRule,
} from '@/lib/port-filter'
import type {
  PortRow,
  PortRule,
  PortScope,
  PortSortKey,
  ProcessDetail,
  ProcessInfo,
  ProcessNode,
  RuntimeInfo,
  SocketProtocolFilter,
  SocketStateFilter,
  SortDirection,
  SortKey,
  ViewMode,
} from '@/types/process'

interface StoreState {
  snapshot: Awaited<ReturnType<typeof api.listProcesses>> | null
  loading: boolean
  refreshing: boolean
  error: string | null
  paused: boolean
  intervalMs: number
  query: string
  portQuery: string
  /** 端口表达式未写 `local:`/`remote:` 时的默认作用域。 */
  portScope: PortScope
  /** 端口视图：套接字状态 / 协议筛选。 */
  socketStateFilter: SocketStateFilter
  socketProtocolFilter: SocketProtocolFilter
  portSortKey: PortSortKey
  portSortDirection: SortDirection
  onlyListening: boolean
  onlyWindowed: boolean
  view: ViewMode
  sortKey: SortKey
  sortDirection: SortDirection
  selectedPid: number | null
  runtime: RuntimeInfo | null
}

export const state = reactive<StoreState>({
  snapshot: null,
  loading: true,
  refreshing: false,
  error: null,
  paused: false,
  intervalMs: 1500,
  query: '',
  portQuery: '',
  portScope: 'both',
  socketStateFilter: 'all',
  socketProtocolFilter: 'all',
  portSortKey: 'port',
  portSortDirection: 'asc',
  onlyListening: false,
  onlyWindowed: false,
  view: 'list',
  sortKey: 'cpu',
  sortDirection: 'desc',
  selectedPid: null,
  runtime: null,
})


/** 详情面板数据（与主列表解耦，独立加载）。 */
export const detail = ref<ProcessDetail | null>(null)
export const detailLoading = ref(false)
export const detailError = ref<string | null>(null)

/** exe 路径 → 图标 DataURL（`null` 表示后端确认没有图标）。 */
const iconCache = reactive(new Map<string, string | null>())
const iconPending = new Set<string>()

/** 进程树展开状态。 */
const expanded = ref(new Set<number>())
let treeInitialized = false

// ---------------------------------------------------------------------------
// 数据获取
// ---------------------------------------------------------------------------

let pollTimer: ReturnType<typeof setTimeout> | null = null
let disposed = false

async function pull(force = false): Promise<void> {
  if (disposed) return
  try {
    const snapshot = force ? await api.refreshProcesses() : await api.listProcesses()
    state.snapshot = snapshot
    state.error = null
    ensureInitialExpansion()
    syncSelection()
  } catch (error) {
    state.error = describeError(error)
  } finally {
    state.loading = false
    state.refreshing = false
  }
}

function schedule(): void {
  if (pollTimer) clearTimeout(pollTimer)
  if (disposed) return
  const delay = state.paused ? 4000 : state.intervalMs
  pollTimer = setTimeout(async () => {
    if (!state.paused) await pull()
    schedule()
  }, delay)
}

/** 启动轮询并做一次首帧加载。 */
export async function bootstrap(): Promise<void> {
  disposed = false
  await Promise.all([pull(), loadRuntime()])
  schedule()
}

/** 页面卸载时停止轮询。 */
export function dispose(): void {
  disposed = true
  if (pollTimer) clearTimeout(pollTimer)
  pollTimer = null
}

async function loadRuntime(): Promise<void> {
  try {
    state.runtime = await api.getRuntimeInfo()
    state.intervalMs = state.runtime.intervalMs || state.intervalMs
  } catch {
    state.runtime = null
  }
}

/** 手动刷新（会请求后端立刻重采）。 */
export async function refreshNow(): Promise<void> {
  state.refreshing = true
  await pull(true)
}

/** 暂停 / 恢复轮询。 */
export function togglePause(): void {
  state.paused = !state.paused
  schedule()
}

/** 修改采样间隔；后端与前端同时生效。 */
export async function changeInterval(millis: number): Promise<void> {
  state.intervalMs = millis
  try {
    const applied = await api.setRefreshInterval(millis)
    state.intervalMs = applied
  } catch {
    /* 浏览器预览环境下忽略 */
  }
  schedule()
}

/** 选中进程并加载详情。 */
export async function select(pid: number | null): Promise<void> {
  state.selectedPid = pid
  detail.value = null
  detailError.value = null
  if (pid == null) return
  detailLoading.value = true
  try {
    detail.value = await api.getProcessDetail(pid)
  } catch (error) {
    detailError.value = describeError(error)
  } finally {
    detailLoading.value = false
  }
}

/** 重新加载当前选中进程的详情（结束进程后调用）。 */
export async function reloadDetail(): Promise<void> {
  if (state.selectedPid != null) await select(state.selectedPid)
}

// ---------------------------------------------------------------------------
// 过滤 / 排序 / 树
// ---------------------------------------------------------------------------

const allProcesses = computed<ProcessInfo[]>(() => state.snapshot?.processes ?? [])

/** 过滤表达式是否处于「激活」状态（决定树是否强制展开）。 */
export const filterActive = computed(
  () => state.query.trim().length > 0 || state.portQuery.trim().length > 0,
)

/** 解析后的端口规则；界面上的「过滤器徽标」直接由它渲染。 */
export const activePortRules = computed<PortRule[]>(() => parsePortQuery(state.portQuery))

/** 端口作用域设置换算成匹配时使用的默认端。 */
const defaultPortScope = computed(() => effectiveScope(state.portScope))

/** 某个具体端口当前是否被「精确规则」过滤（用于高亮端口徽标）。 */
const exactFilteredPorts = computed(() => {
  const set = new Set<number>()
  for (const rule of activePortRules.value) {
    if (rule.from === rule.to) set.add(rule.from)
  }
  return set
})

export function isPortFiltered(port: number): boolean {
  return exactFilteredPorts.value.has(port)
}

/** 进程是否命中端口过滤（其任一端口/远端端口落在任一规则内）。 */
function processMatchesPortRules(process: ProcessInfo): boolean {
  const rules = activePortRules.value
  if (rules.length === 0) return true
  const scope = defaultPortScope.value
  return process.sockets.some((socket) => socketMatchesAnyRule(socket, rules, scope))
}

/** 名称 / PID / 命令行 / 路径 模糊匹配 + 端口匹配 + 快捷开关。 */
export const filteredProcesses = computed<ProcessInfo[]>(() => {
  const keyword = state.query.trim().toLowerCase()
  let list = allProcesses.value

  if (state.onlyListening) list = list.filter((p) => p.listeningPorts.length > 0)
  if (state.onlyWindowed) list = list.filter((p) => p.windowTitles.length > 0)

  if (keyword) {
    list = list.filter((p) => {
      if (p.displayName.toLowerCase().includes(keyword)) return true
      if (p.name.toLowerCase().includes(keyword)) return true
      if (String(p.pid) === keyword || String(p.pid).includes(keyword)) return true
      if (p.exe && p.exe.toLowerCase().includes(keyword)) return true
      if (p.cmdLine.toLowerCase().includes(keyword)) return true
      if (p.user && p.user.toLowerCase().includes(keyword)) return true
      return p.windowTitles.some((title) => title.toLowerCase().includes(keyword))
    })
  }

  if (activePortRules.value.length > 0) {
    list = list.filter(processMatchesPortRules)
  }

  return sortProcesses(list, state.sortKey, state.sortDirection)
})

/** 列表视图的数据源。 */
export const visibleProcesses = computed(() => filteredProcesses.value)

/** 树视图中有匹配结果的 pid 集合（用于把祖先置灰）。 */
const matchedPids = computed(() => new Set(filteredProcesses.value.map((p) => p.pid)))

/** 树视图的数据源：无过滤时用全量，有过滤时补上祖先链。 */
const treeSource = computed<ProcessInfo[]>(() => {
  if (!filterActive.value && !state.onlyListening && !state.onlyWindowed) {
    return allProcesses.value
  }
  const keep = new Map<number, ProcessInfo>()
  const byPid = new Map(allProcesses.value.map((p) => [p.pid, p]))
  for (const process of filteredProcesses.value) {
    let cursor: ProcessInfo | undefined = process
    let guard = 0
    while (cursor && guard < 64) {
      keep.set(cursor.pid, cursor)
      cursor = cursor.parentPid != null ? byPid.get(cursor.parentPid) : undefined
      guard += 1
    }
  }
  return [...keep.values()]
})

export const treeRoots = computed<ProcessNode[]>(() =>
  buildTree(treeSource.value, state.sortKey, state.sortDirection),
)

/** 展平后的树行，交给表格渲染。 */
export const treeRows = computed<ProcessNode[]>(() => {
  const rows: ProcessNode[] = []
  const forceExpand = filterActive.value || state.onlyListening || state.onlyWindowed
  const walk = (nodes: ProcessNode[]): void => {
    for (const node of nodes) {
      rows.push(node)
      if (node.children.length > 0 && (forceExpand || expanded.value.has(node.process.pid))) {
        walk(node.children)
      }
    }
  }
  walk(treeRoots.value)
  return rows
})

export function isMatched(pid: number): boolean {
  return matchedPids.value.has(pid)
}

export function isExpanded(pid: number): boolean {
  return expanded.value.has(pid)
}

export function toggleExpand(pid: number): void {
  const next = new Set(expanded.value)
  if (next.has(pid)) next.delete(pid)
  else next.add(pid)
  expanded.value = next
}

export function expandAll(): void {
  const next = new Set<number>()
  for (const process of allProcesses.value) {
    if (process.children.length > 0) next.add(process.pid)
  }
  expanded.value = next
}

export function collapseAll(): void {
  expanded.value = new Set<number>()
}

/** 让列表按某一列排序；重复点击同一列时翻转方向。 */
export function toggleSort(key: SortKey): void {
  if (state.sortKey === key) {
    state.sortDirection = state.sortDirection === 'asc' ? 'desc' : 'asc'
  } else {
    state.sortKey = key
    // 文本类默认升序，数值类默认降序（先看占用最高的进程）。
    state.sortDirection = key === 'name' || key === 'user' || key === 'status' ? 'asc' : 'desc'
  }
}

export function resetFilters(): void {
  state.query = ''
  state.portQuery = ''
  state.portScope = 'both'
  state.onlyListening = false
  state.onlyWindowed = false
  state.socketStateFilter = 'all'
  state.socketProtocolFilter = 'all'
}

// ---------------------------------------------------------------------------
// 端口视图（每条套接字一行）
// ---------------------------------------------------------------------------

/** 全部套接字，附带宿主进程信息。 */
export const allPortRows = computed<PortRow[]>(() => {
  const rows: PortRow[] = []
  for (const process of allProcesses.value) {
    for (const socket of process.sockets) {
      rows.push({
        key: [
          process.pid,
          socket.protocol,
          socket.family,
          socket.localAddr,
          socket.localPort,
          socket.remoteAddr ?? '',
          socket.remotePort ?? 0,
          socket.state,
        ].join('|'),
        pid: process.pid,
        name: process.name,
        displayName: process.displayName,
        exe: process.exe,
        user: process.user,
        socket,
      })
    }
  }
  return rows
})

/** 端口视图的过滤结果：端口表达式 + 状态 + 协议 + 名称关键词。 */
export const filteredPortRows = computed<PortRow[]>(() => {
  const keyword = state.query.trim().toLowerCase()
  const rules = activePortRules.value
  const scope = defaultPortScope.value
  let list = allPortRows.value

  if (state.socketProtocolFilter !== 'all') {
    list = list.filter((row) => row.socket.protocol === state.socketProtocolFilter)
  }
  if (state.socketStateFilter === 'listen') {
    list = list.filter((row) => row.socket.listening)
  } else if (state.socketStateFilter === 'active') {
    list = list.filter((row) => !row.socket.listening)
  }
  if (state.onlyListening) list = list.filter((row) => row.socket.listening)

  if (keyword) {
    list = list.filter(
      (row) =>
        row.displayName.toLowerCase().includes(keyword) ||
        row.name.toLowerCase().includes(keyword) ||
        String(row.pid).includes(keyword) ||
        (row.exe ?? '').toLowerCase().includes(keyword) ||
        String(row.socket.localPort).includes(keyword),
    )
  }

  if (rules.length > 0) {
    list = list.filter((row) => socketMatchesAnyRule(row.socket, rules, scope))
  }

  return sortPortRows(list, state.portSortKey, state.portSortDirection)
})

/** 端口视图汇总：总数 / 监听数 / 活跃连接数 / 涉及进程数。 */
export const portSummary = computed(() => {
  const rows = filteredPortRows.value
  let listening = 0
  let active = 0
  const pids = new Set<number>()
  for (const row of rows) {
    if (row.socket.listening) listening += 1
    else active += 1
    pids.add(row.pid)
  }
  return { total: rows.length, listening, active, processes: pids.size }
})

/** 端口视图排序；重复点击同一列翻转方向。 */
export function togglePortSort(key: PortSortKey): void {
  if (state.portSortKey === key) {
    state.portSortDirection = state.portSortDirection === 'asc' ? 'desc' : 'asc'
  } else {
    // 端口视图默认都是「从小到大 / A→Z」，换列时重置为升序。
    state.portSortKey = key
    state.portSortDirection = 'asc'
  }
}

/**
 * 增 / 删一条「精确端口」过滤规则。
 *
 * 供用户点击端口数字使用：点一次加入过滤，再点一次移除。
 * `scope` 默认 `any`（跟随工具栏的作用域设置）；从「远程地址」列点击时
 * 传 `'remote'`，这样即便工具栏选了「仅本地端口」也能立刻命中。
 * 移除时不区分协议与作用域，因此点击 UDP 行上的 53 也能取消 `tcp:53`。
 */
export function togglePortFilter(
  port: number,
  protocol?: 'TCP' | 'UDP',
  scope: 'local' | 'remote' | 'any' = 'any',
): void {
  const rules = activePortRules.value
  const index = rules.findIndex((rule) => rule.from === port && rule.to === port)

  const next: PortRule[] =
    index >= 0
      ? rules.filter((_, i) => i !== index)
      : [
          ...rules,
          {
            token: String(port),
            protocol: protocol ?? null,
            scope,
            from: port,
            to: port,
          },
        ]

  state.portQuery = next.map(ruleToToken).join(', ')
}

/** 移除某条已生效的端口规则（工具栏上的徽标 ×）。 */
export function removePortRule(token: string): void {
  const next = activePortRules.value.filter((rule) => rule.token !== token)
  state.portQuery = next.map(ruleToToken).join(', ')
}

function sortPortRows(list: PortRow[], key: PortSortKey, direction: SortDirection): PortRow[] {
  const factor = direction === 'asc' ? 1 : -1
  return [...list].sort((a, b) => {
    let diff = 0
    switch (key) {
      case 'port':
        diff = a.socket.localPort - b.socket.localPort
        break
      case 'protocol':
        diff = a.socket.protocol.localeCompare(b.socket.protocol)
        break
      case 'state':
        diff = a.socket.state.localeCompare(b.socket.state)
        break
      case 'process':
        diff = a.displayName.localeCompare(b.displayName, 'zh-Hans-CN')
        break
      default:
        diff = 0
    }
    if (diff !== 0) return diff * factor
    return a.socket.localPort - b.socket.localPort
  })
}

// ---------------------------------------------------------------------------
// 图标
// ---------------------------------------------------------------------------

export function iconFor(exe: string | null): string | null {
  if (!exe) return null
  if (iconCache.has(exe)) return iconCache.get(exe) ?? null
  if (!iconPending.has(exe)) {
    iconPending.add(exe)
    api
      .getProcessIcon(exe)
      .then((data) => iconCache.set(exe, data ?? null))
      .catch(() => iconCache.set(exe, null))
      .finally(() => iconPending.delete(exe))
  }
  return null
}

// ---------------------------------------------------------------------------
// 写操作
// ---------------------------------------------------------------------------

/** 后端错误统一序列化为 `{ code, message }`，这里做一次归一化。 */
function describeError(error: unknown): string {
  if (typeof error === 'string') return error
  if (error && typeof error === 'object' && 'message' in error) {
    const message = (error as { message?: unknown }).message
    if (typeof message === 'string') return message
  }
  if (error instanceof Error) return error.message
  return String(error ?? '未知错误')
}

/** 结束单个进程。 */
export async function killProcess(pid: number, force = true): Promise<boolean> {
  try {
    await api.killProcess(pid, force)
    toast.success(`已结束进程 ${pid}`)
    if (state.selectedPid === pid) await select(null)
    await pull(true)
    return true
  } catch (error) {
    toast.error('结束进程失败', { description: describeError(error) })
    return false
  }
}

/** 结束整棵进程树。 */
export async function killProcessTree(pid: number, force = true): Promise<boolean> {
  try {
    const result = await api.killProcessTree(pid, force)
    if (result.failed.length > 0) {
      toast.warning(`已结束 ${result.killed.length} 个进程，${result.failed.length} 个失败`, {
        description: result.failed
          .slice(0, 3)
          .map((f) => `${f.name}(${f.pid})`)
          .join('、'),
      })
    } else {
      toast.success(`已结束 ${result.killed.length} 个进程`)
    }
    if (state.selectedPid != null && result.killed.includes(state.selectedPid)) {
      await select(null)
    } else {
      await reloadDetail()
    }
    await pull(true)
    return true
  } catch (error) {
    toast.error('结束进程树失败', { description: describeError(error) })
    return false
  }
}

/** 在资源管理器中定位可执行文件。 */
export async function reveal(exe: string | null): Promise<void> {
  if (!exe) {
    toast.warning('该进程没有可用的可执行文件路径')
    return
  }
  try {
    await api.revealInExplorer(exe)
  } catch (error) {
    toast.error('无法打开文件位置', { description: describeError(error) })
  }
}

/** 把进程主窗口切到前台。 */
export async function focusWindow(pid: number): Promise<void> {
  try {
    const ok = await api.focusProcessWindow(pid)
    if (!ok) toast.warning('该进程没有可切换到前台的可见窗口')
  } catch (error) {
    toast.error('切换窗口失败', { description: describeError(error) })
  }
}

/** 复制文本到剪贴板，并给出统一反馈。 */
export async function copyText(text: string, label = '内容'): Promise<void> {
  if (!text) {
    toast.warning(`没有可复制的${label}`)
    return
  }
  try {
    await navigator.clipboard.writeText(text)
    toast.success(`已复制${label}`)
  } catch {
    toast.error('复制失败：浏览器拒绝了剪贴板访问')
  }
}

// ---------------------------------------------------------------------------
// 内部工具
// ---------------------------------------------------------------------------

function ensureInitialExpansion(): void {
  if (treeInitialized || allProcesses.value.length === 0) return
  treeInitialized = true
  // 初次进入树视图时展开根节点与第二层，避免一屏全是折叠箭头。
  const next = new Set<number>()
  const byPid = new Map(allProcesses.value.map((p) => [p.pid, p]))
  for (const process of allProcesses.value) {
    if (process.children.length === 0) continue
    const parentId = process.parentPid
    const hasParent = parentId != null && byPid.has(parentId)
    if (!hasParent) next.add(process.pid)
  }
  for (const process of allProcesses.value) {
    const parentId = process.parentPid
    if (parentId != null && next.has(parentId)) next.add(process.pid)
  }
  expanded.value = next
}

/** 选中的进程已退出时清理选中态。 */
function syncSelection(): void {
  if (state.selectedPid == null) return
  const exists = allProcesses.value.some((p) => p.pid === state.selectedPid)
  if (!exists) {
    state.selectedPid = null
    detail.value = null
  }
}

function sortProcesses(list: ProcessInfo[], key: SortKey, direction: SortDirection): ProcessInfo[] {
  const factor = direction === 'asc' ? 1 : -1
  return [...list].sort((a, b) => {
    const diff = compareBy(a, b, key)
    if (diff !== 0) return diff * factor
    return a.pid - b.pid
  })
}

function compareBy(a: ProcessInfo, b: ProcessInfo, key: SortKey): number {
  switch (key) {
    case 'name':
      return a.displayName.localeCompare(b.displayName, 'zh-Hans-CN')
    case 'pid':
      return a.pid - b.pid
    case 'cpu':
      return a.cpuUsage - b.cpuUsage
    case 'memory':
      return a.memory - b.memory
    case 'ports': {
      const left = a.listeningPorts[0] ?? a.ports[0] ?? Number.MAX_SAFE_INTEGER
      const right = b.listeningPorts[0] ?? b.ports[0] ?? Number.MAX_SAFE_INTEGER
      return left - right
    }
    case 'threads':
      return a.threadCount - b.threadCount
    case 'startTime':
      return a.startTime - b.startTime
    case 'user':
      return (a.user ?? '').localeCompare(b.user ?? '', 'zh-Hans-CN')
    case 'status':
      return a.status.localeCompare(b.status)
    default:
      return 0
  }
}

function buildTree(
  list: ProcessInfo[],
  key: SortKey,
  direction: SortDirection,
): ProcessNode[] {
  const nodes = new Map<number, ProcessNode>()
  for (const process of list) {
    nodes.set(process.pid, {
      process,
      depth: 0,
      children: [],
      hasChildren: false,
    })
  }

  const roots: ProcessNode[] = []
  for (const node of nodes.values()) {
    const parentId = node.process.parentPid
    const parent = parentId != null ? nodes.get(parentId) : undefined
    if (parent && parent !== node) parent.children.push(node)
    else roots.push(node)
  }

  const sortNodes = (items: ProcessNode[], depth: number): void => {
    items.sort((a, b) => {
      const diff = compareBy(a.process, b.process, key)
      if (diff !== 0) return diff * (direction === 'asc' ? 1 : -1)
      return a.process.pid - b.process.pid
    })
    for (const item of items) {
      item.depth = depth
      item.hasChildren = item.children.length > 0
      sortNodes(item.children, depth + 1)
    }
  }
  sortNodes(roots, 0)
  return roots
}
