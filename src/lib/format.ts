/** 纯展示层的格式化工具，全部无副作用。 */

const BYTE_UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB']

/** 人类可读的字节数，例如 `1.24 GB`。 */
export function formatBytes(bytes: number, digits = 1): string {
  if (!Number.isFinite(bytes) || bytes <= 0) return '0 B'
  let value = bytes
  let unit = 0
  while (value >= 1024 && unit < BYTE_UNITS.length - 1) {
    value /= 1024
    unit += 1
  }
  const fixed = unit === 0 ? 0 : digits
  return `${value.toFixed(fixed)} ${BYTE_UNITS[unit]}`
}

/** 千分位整数，例如 `12,480`。 */
export function formatNumber(value: number): string {
  if (!Number.isFinite(value)) return '-'
  return Math.round(value).toLocaleString('en-US')
}

/** CPU 百分比，保留一位小数。 */
export function formatPercent(value: number, digits = 1): string {
  if (!Number.isFinite(value)) return '-'
  return `${value.toFixed(digits)}%`
}

/** 秒 → `3 天 4 小时` / `12 分 05 秒`。 */
export function formatDuration(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return '-'
  const total = Math.floor(seconds)
  const days = Math.floor(total / 86400)
  const hours = Math.floor((total % 86400) / 3600)
  const minutes = Math.floor((total % 3600) / 60)
  const secs = total % 60
  const parts: string[] = []
  if (days > 0) parts.push(`${days} 天`)
  if (hours > 0) parts.push(`${hours} 小时`)
  if (minutes > 0 && days === 0) parts.push(`${minutes} 分`)
  if (parts.length === 0) parts.push(`${secs} 秒`)
  return parts.join(' ')
}

/** Unix 秒 → 本地时间字符串。 */
export function formatDateTime(unixSeconds: number): string {
  if (!Number.isFinite(unixSeconds) || unixSeconds <= 0) return '-'
  const date = new Date(unixSeconds * 1000)
  const pad = (n: number) => String(n).padStart(2, '0')
  return (
    `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ` +
    `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
  )
}

/** Unix 毫秒 → `HH:mm:ss`。 */
export function formatClock(unixMillis: number): string {
  if (!unixMillis) return '-'
  const date = new Date(unixMillis)
  const pad = (n: number) => String(n).padStart(2, '0')
  return `${pad(date.getHours())}:${pad(date.getMinutes())}:${pad(date.getSeconds())}`
}

/** 进程状态码 → 中文标签。 */
const STATUS_LABELS: Record<string, string> = {
  Run: '运行中',
  Sleep: '休眠',
  Stop: '已停止',
  Zombie: '僵尸',
  Idle: '空闲',
  Tracing: '调试中',
  Dead: '已结束',
  Wakekill: '唤醒终止',
  Waking: '唤醒中',
  Parked: '已挂起',
  LockBlocked: '锁等待',
  UninterruptibleDiskSleep: '不可中断',
  Unknown: '未知',
}

export function statusLabel(status: string): string {
  return STATUS_LABELS[status] ?? status
}

/** 截断过长的命令行，保留头部与尾部。 */
export function truncateMiddle(text: string, max = 120): string {
  if (text.length <= max) return text
  const head = Math.ceil((max - 3) * 0.6)
  const tail = max - 3 - head
  return `${text.slice(0, head)}...${text.slice(text.length - tail)}`
}
