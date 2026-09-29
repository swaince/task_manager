/**
 * 端口过滤表达式的解析与匹配。
 *
 * 支持三种可选前缀，可叠加：
 *
 * | 写法 | 含义 |
 * | --- | --- |
 * | `8080` | 端口等于 8080 |
 * | `80,443` | 多个端口（也支持空格、中文逗号、分号分隔） |
 * | `8000-8100` | 端口区间 |
 * | `tcp:8080` | 限定协议（`tcp:` / `udp:`） |
 * | `local:8080` | 只看本地端口（`local:` / `l:`） |
 * | `remote:443` | 只看远端端口（`remote:` / `r:`） |
 * | `tcp:local:8000-8100` | 前缀可叠加 |
 *
 * 未写 `local:` / `remote:` 时，按调用方传入的「端口作用域」决定匹配哪一端。
 */

import type { PortRule, PortScope, SocketInfo } from '@/types/process'

/** 分隔符：英文/中文逗号、分号、空白。 */
const SEPARATORS = /[,，;；、\s]+/

/**
 * 解析端口过滤表达式。无法识别的 token 会被忽略（不报错），
 * 这样用户边输入边过滤的体验不会被半成品输入打断。
 */
export function parsePortQuery(raw: string): PortRule[] {
  const rules: PortRule[] = []
  const seen = new Set<string>()

  for (const chunk of raw.split(SEPARATORS)) {
    const token = chunk.trim()
    if (!token) continue

    const rule = parseToken(token)
    if (!rule) continue
    if (seen.has(rule.token)) continue
    seen.add(rule.token)
    rules.push(rule)
  }
  return rules
}

function parseToken(token: string): PortRule | null {
  let rest = token
  let protocol: PortRule['protocol'] = null
  let scope: PortRule['scope'] = 'any'

  // 前缀最多剥两层，顺序不限。
  for (let depth = 0; depth < 2; depth += 1) {
    const match = rest.match(/^([a-zA-Z]+):(.*)$/)
    if (!match) break
    const prefix = match[1].toLowerCase()
    const remainder = match[2]
    switch (prefix) {
      case 'tcp':
        protocol = 'TCP'
        break
      case 'udp':
        protocol = 'UDP'
        break
      case 'local':
      case 'l':
        scope = 'local'
        break
      case 'remote':
      case 'r':
        scope = 'remote'
        break
      default:
        // 未知前缀：整体丢弃，避免把 `http:80` 误判成端口。
        return null
    }
    rest = remainder
  }

  if (!rest) return null

  const range = rest.match(/^(\d{1,5})\s*[-~]\s*(\d{1,5})$/)
  if (range) {
    let from = Number(range[1])
    let to = Number(range[2])
    if (from > to) [from, to] = [to, from]
    if (!isPort(from) || !isPort(to)) return null
    // 防御：区间过大时截断，避免构造出巨量规则。
    if (to - from > 1024) to = Math.min(65535, from + 1024)
    return { token, protocol, scope, from, to }
  }

  if (/^\d{1,5}$/.test(rest)) {
    const port = Number(rest)
    if (!isPort(port)) return null
    return { token, protocol, scope, from: port, to: port }
  }

  return null
}

function isPort(value: number): boolean {
  return Number.isInteger(value) && value > 0 && value <= 65535
}

/** 规则 → 规范化 token（点击端口徽标增删过滤器时用来回写输入框）。 */
export function ruleToToken(rule: PortRule): string {
  const parts: string[] = []
  if (rule.protocol) parts.push(rule.protocol.toLowerCase())
  if (rule.scope !== 'any') parts.push(rule.scope)
  parts.push(rule.from === rule.to ? String(rule.from) : `${rule.from}-${rule.to}`)
  return parts.join(':')
}

/** 规则的可读描述，用于提示气泡。 */
export function describeRule(rule: PortRule): string {
  const port =
    rule.from === rule.to ? `端口 ${rule.from}` : `端口 ${rule.from}-${rule.to}`
  const protocol = rule.protocol ? `${rule.protocol} ` : ''
  const scope =
    rule.scope === 'local' ? '本地' : rule.scope === 'remote' ? '远端' : '本地或远端'
  return `${protocol}${scope} ${port}`
}

/** 把界面上的作用域选择换算成匹配时使用的默认作用域。 */
export function effectiveScope(scope: PortScope): 'local' | 'remote' | 'any' {
  return scope === 'both' ? 'any' : scope
}

/** 单条套接字是否命中某条规则。 */
export function socketMatchesRule(
  socket: SocketInfo,
  rule: PortRule,
  defaultScope: 'local' | 'remote' | 'any',
): boolean {
  if (rule.protocol && socket.protocol !== rule.protocol) return false

  const scope = rule.scope === 'any' ? defaultScope : rule.scope
  const localHit = socket.localPort >= rule.from && socket.localPort <= rule.to
  const remoteHit =
    socket.remotePort != null && socket.remotePort >= rule.from && socket.remotePort <= rule.to

  if (scope === 'local') return localHit
  if (scope === 'remote') return remoteHit
  return localHit || remoteHit
}

/** 一组规则中任意一条命中即可。空规则集视为「不过滤」。 */
export function socketMatchesAnyRule(
  socket: SocketInfo,
  rules: PortRule[],
  defaultScope: 'local' | 'remote' | 'any',
): boolean {
  if (rules.length === 0) return true
  return rules.some((rule) => socketMatchesRule(socket, rule, defaultScope))
}
