import { describe, expect, it } from 'vitest'

import {
  describeRule,
  effectiveScope,
  parsePortQuery,
  ruleToToken,
  socketMatchesAnyRule,
  socketMatchesRule,
} from '@/lib/port-filter'
import type { PortRule, SocketInfo } from '@/types/process'

function tcp(local: number, remote?: number, addr = '127.0.0.1'): SocketInfo {
  return {
    protocol: 'TCP',
    family: 'IPv4',
    localAddr: addr,
    localPort: local,
    remoteAddr: remote == null ? null : '10.0.0.1',
    remotePort: remote ?? null,
    state: remote == null ? 'LISTEN' : 'ESTABLISHED',
    listening: remote == null,
  }
}

function udp(local: number): SocketInfo {
  return {
    protocol: 'UDP',
    family: 'IPv4',
    localAddr: '0.0.0.0',
    localPort: local,
    remoteAddr: null,
    remotePort: null,
    state: 'BOUND',
    listening: true,
  }
}

describe('parsePortQuery', () => {
  it('解析单个端口', () => {
    expect(parsePortQuery('8080')).toEqual<PortRule[]>([
      { token: '8080', protocol: null, scope: 'any', from: 8080, to: 8080 },
    ])
  })

  it('支持多种分隔符', () => {
    const tokens = parsePortQuery('80,443 8080；3000、5000').map((r) => r.token)
    expect(tokens).toEqual(['80', '443', '8080', '3000', '5000'])
  })

  it('解析区间并把逆序区间纠正过来', () => {
    expect(parsePortQuery('8000-8100')[0]).toMatchObject({ from: 8000, to: 8100 })
    expect(parsePortQuery('9000-8000')[0]).toMatchObject({ from: 8000, to: 9000 })
  })

  it('解析协议与作用域前缀，顺序不限', () => {
    expect(parsePortQuery('tcp:8080')[0]).toMatchObject({ protocol: 'TCP', scope: 'any' })
    expect(parsePortQuery('udp:53')[0]).toMatchObject({ protocol: 'UDP', scope: 'any' })
    expect(parsePortQuery('local:3000')[0]).toMatchObject({ scope: 'local' })
    expect(parsePortQuery('remote:443')[0]).toMatchObject({ scope: 'remote' })
    expect(parsePortQuery('tcp:local:8000-8100')[0]).toMatchObject({
      protocol: 'TCP',
      scope: 'local',
      from: 8000,
      to: 8100,
    })
    expect(parsePortQuery('remote:udp:53')[0]).toMatchObject({
      protocol: 'UDP',
      scope: 'remote',
    })
  })

  it('忽略无法识别的 token，而不是报错', () => {
    expect(parsePortQuery('abc http:80 8080')).toHaveLength(1)
    expect(parsePortQuery('99999 0 -5')).toHaveLength(0)
  })

  it('去重', () => {
    expect(parsePortQuery('8080,8080 , 8080')).toHaveLength(1)
  })

  it('空表达式返回空规则集', () => {
    expect(parsePortQuery('')).toEqual([])
    expect(parsePortQuery('   ')).toEqual([])
  })

  it('超大区间会被截断，避免构造巨量规则', () => {
    const rule = parsePortQuery('1-65000')[0]
    expect(rule.to).toBeLessThanOrEqual(rule.from + 1024)
  })
})

describe('ruleToToken / describeRule', () => {
  it('规范化回写', () => {
    expect(ruleToToken({ token: '', protocol: 'TCP', scope: 'local', from: 8080, to: 8080 })).toBe(
      'tcp:local:8080',
    )
    expect(ruleToToken({ token: '', protocol: null, scope: 'any', from: 80, to: 90 })).toBe('80-90')
  })

  it('人话描述', () => {
    expect(
      describeRule({ token: '', protocol: 'UDP', scope: 'remote', from: 53, to: 53 }),
    ).toContain('UDP')
  })
})

describe('socketMatchesRule', () => {
  const listen = tcp(8080)
  const conn = tcp(51234, 443)

  it('默认作用域为 any 时本地与远端都算命中', () => {
    const rule = parsePortQuery('443')[0]
    expect(socketMatchesRule(conn, rule, 'any')).toBe(true)
    expect(socketMatchesRule(listen, rule, 'any')).toBe(false)
  })

  it('scope=local 只匹配本地端口', () => {
    const rule = parsePortQuery('443')[0]
    expect(socketMatchesRule(conn, rule, 'local')).toBe(false)
    expect(socketMatchesRule(listen, parsePortQuery('8080')[0], 'local')).toBe(true)
  })

  it('scope=remote 只匹配远端端口', () => {
    const rule = parsePortQuery('443')[0]
    expect(socketMatchesRule(conn, rule, 'remote')).toBe(true)
    expect(socketMatchesRule(tcp(443), rule, 'remote')).toBe(false)
  })

  it('规则自带作用域时覆盖默认值', () => {
    const rule = parsePortQuery('remote:443')[0]
    expect(socketMatchesRule(conn, rule, 'local')).toBe(true)
  })

  it('协议前缀生效', () => {
    expect(socketMatchesRule(udp(53), parsePortQuery('tcp:53')[0], 'any')).toBe(false)
    expect(socketMatchesRule(udp(53), parsePortQuery('udp:53')[0], 'any')).toBe(true)
    expect(socketMatchesRule(udp(53), parsePortQuery('53')[0], 'any')).toBe(true)
  })

  it('区间匹配', () => {
    const rule = parsePortQuery('8000-8100')[0]
    expect(socketMatchesRule(tcp(8080), rule, 'any')).toBe(true)
    expect(socketMatchesRule(tcp(7999), rule, 'any')).toBe(false)
  })

  it('UDP 没有远端端口，remote 规则永不命中', () => {
    expect(socketMatchesRule(udp(53), parsePortQuery('remote:53')[0], 'any')).toBe(false)
  })
})

describe('socketMatchesAnyRule', () => {
  it('空规则集视为不过滤', () => {
    expect(socketMatchesAnyRule(tcp(1234), [], 'any')).toBe(true)
  })

  it('任意一条命中即可', () => {
    const rules = parsePortQuery('80, 443')
    expect(socketMatchesAnyRule(tcp(443), rules, 'any')).toBe(true)
    expect(socketMatchesAnyRule(tcp(8080), rules, 'any')).toBe(false)
  })
})

describe('effectiveScope', () => {
  it('both 换算成 any', () => {
    expect(effectiveScope('both')).toBe('any')
    expect(effectiveScope('local')).toBe('local')
    expect(effectiveScope('remote')).toBe('remote')
  })
})
