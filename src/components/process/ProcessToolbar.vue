<script setup lang="ts">
/**
 * 过滤与视图工具栏。
 *
 * 名称框支持：进程名、PID、可执行路径、命令行、用户名、窗口标题、端口号。
 * 端口框支持的表达式见 `lib/port-filter.ts`：
 * `8080` / `80,443` / `8000-8100` / `tcp:8080` / `local:8080` / `remote:443`。
 */
import { computed } from 'vue'
import {
  ChevronsDownUpIcon,
  ChevronsUpDownIcon,
  FilterXIcon,
  ListTreeIcon,
  SearchIcon,
  TableIcon,
  WaypointsIcon,
  XIcon,
} from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import {
  activePortRules,
  collapseAll,
  expandAll,
  filteredProcesses,
  filterActive,
  portSummary,
  removePortRule,
  resetFilters,
  state,
} from '@/composables/useProcessStore'
import { describeRule, ruleToToken } from '@/lib/port-filter'
import { formatNumber } from '@/lib/format'
import type { PortScope, SocketProtocolFilter, SocketStateFilter, ViewMode } from '@/types/process'

const processCount = computed(() => state.snapshot?.totals.processCount ?? 0)
const visibleCount = computed(() => filteredProcesses.value.length)
const socketCount = computed(() => state.snapshot?.totals.connectionCount ?? 0)
const portsSummary = computed(() => portSummary.value)
const rules = computed(() => activePortRules.value)

const views: { value: ViewMode; label: string; icon: unknown }[] = [
  { value: 'list', label: '列表', icon: TableIcon },
  { value: 'tree', label: '进程树', icon: ListTreeIcon },
  { value: 'ports', label: '端口', icon: WaypointsIcon },
]

const scopeHint = computed(() => {
  switch (state.portScope) {
    case 'local':
      return '只看本机监听 / 占用的端口，回答「谁占用了这个端口」。例：输入 443 只命中本机监听 443 的服务。'
    case 'remote':
      return '只看本机连出去的远端端口，回答「谁在访问这个端口」。例：输入 443 只命中正在连接远端 443 的进程（UDP 没有远端端口，不会命中）。'
    default:
      return '同一个端口号：本机占用它、或本机连到远端的它，都算命中（结果最多，可能需要再收紧）。例：输入 443 会同时列出本机 443 服务与所有连远端 443 的进程。'
  }
})

function setView(view: ViewMode): void {
  state.view = view
}

function onScopeChange(value: unknown): void {
  const scope = String(value) as PortScope
  if (scope === 'local' || scope === 'remote' || scope === 'both') state.portScope = scope
}

/** 规则 token → 展示文本，带上协议 / 作用域前缀。 */
function ruleLabel(token: string): string {
  const rule = rules.value.find((item) => item.token === token)
  return rule ? ruleToToken(rule) : token
}

function ruleTooltip(token: string): string {
  const rule = rules.value.find((item) => item.token === token)
  return rule ? describeRule(rule) : token
}

const hasAnyFilter = computed(
  () =>
    filterActive.value ||
    state.onlyListening ||
    state.onlyWindowed ||
    state.socketStateFilter !== 'all' ||
    state.socketProtocolFilter !== 'all' ||
    state.portScope !== 'both',
)

const scopeText = computed(() => {
  switch (state.portScope) {
    case 'local':
      return '仅本地端口'
    case 'remote':
      return '仅远端端口'
    default:
      return '本地或远端'
  }
})

/** 端口视图的两个维度筛选：用分段按钮而不是下拉，和「视图切换」样式统一、也更省横向空间。 */
const socketStates: { value: SocketStateFilter; label: string }[] = [
  { value: 'all', label: '全部' },
  { value: 'listen', label: '仅监听' },
  { value: 'active', label: '仅活跃' },
]

const socketProtocols: { value: SocketProtocolFilter; label: string }[] = [
  { value: 'all', label: '全部' },
  { value: 'TCP', label: 'TCP' },
  { value: 'UDP', label: 'UDP' },
]

function setSocketState(value: SocketStateFilter): void {
  state.socketStateFilter = value
}

function setSocketProtocol(value: SocketProtocolFilter): void {
  state.socketProtocolFilter = value
}
</script>

<template>
  <div class="shrink-0 border-b bg-background">
    <!-- 第一行：搜索 + 端口表达式 + 视图切换 -->
    <div class="flex flex-wrap items-center gap-2 px-3 py-2">
      <div class="relative min-w-[190px] flex-1 md:max-w-[300px]">
        <SearchIcon
          class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
        />
        <Input
          v-model="state.query"
          class="h-8 pl-8 text-sm"
          placeholder="搜索名称 / PID / 路径 / 命令行 / 窗口标题"
          spellcheck="false"
        />
      </div>

      <!-- 端口过滤 -->
      <div class="relative w-[200px]">
        <WaypointsIcon
          class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
        />
        <Input
          v-model="state.portQuery"
          class="h-8 pr-7 pl-8 font-mono text-sm"
          placeholder="端口 8080, 8000-8100"
          spellcheck="false"
        />
        <button
          v-if="state.portQuery"
          type="button"
          class="absolute top-1/2 right-1.5 -translate-y-1/2 rounded-sm p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
          title="清空端口过滤"
          @click="state.portQuery = ''"
        >
          <XIcon class="size-3.5" />
        </button>
      </div>

      <Tooltip>
        <TooltipTrigger as-child>
          <Select :model-value="state.portScope" @update:model-value="onScopeChange">
            <SelectTrigger size="sm" class="w-[124px]">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="both">本地+远端</SelectItem>
              <SelectItem value="local">仅本地端口</SelectItem>
              <SelectItem value="remote">仅远端端口</SelectItem>
            </SelectContent>
          </Select>
        </TooltipTrigger>
        <TooltipContent side="bottom" class="max-w-sm text-xs leading-relaxed">
          <p class="mb-1 font-medium">端口匹配范围</p>
          <p>{{ scopeHint }}</p>
          <p class="mt-1 text-muted-foreground">
            表达式里写 <code class="font-mono">local:</code> /
            <code class="font-mono">remote:</code> 可单独覆盖这一项。
          </p>
        </TooltipContent>
      </Tooltip>

      <!--
        快捷筛选：属于「主过滤」，与搜索/端口同处一行。
        list / tree 视图下是「仅监听端口 / 仅有窗口」；
        端口视图下换成等价的「套接字状态 / 协议」分段按钮（语义由套接字承担）。
      -->
      <template v-if="state.view !== 'ports'">
        <label
          class="flex h-8 cursor-pointer items-center gap-2 rounded-md border px-2 text-xs select-none"
          :class="state.onlyListening ? 'border-emerald-500/40 bg-emerald-500/10' : 'border-border'"
        >
          <Switch
            :model-value="state.onlyListening"
            size="sm"
            @update:model-value="(value: unknown) => (state.onlyListening = Boolean(value))"
          />
          仅监听端口
        </label>

        <label
          class="flex h-8 cursor-pointer items-center gap-2 rounded-md border px-2 text-xs select-none"
          :class="state.onlyWindowed ? 'border-sky-500/40 bg-sky-500/10' : 'border-border'"
        >
          <Switch
            :model-value="state.onlyWindowed"
            size="sm"
            @update:model-value="(value: unknown) => (state.onlyWindowed = Boolean(value))"
          />
          仅有窗口
        </label>
      </template>

      <template v-else>
        <Tooltip>
          <TooltipTrigger as-child>
            <div class="flex items-center gap-1.5">
              <span class="text-[11px] text-muted-foreground">状态</span>
              <div class="flex items-center rounded-md border p-0.5">
                <Button
                  v-for="item in socketStates"
                  :key="item.value"
                  size="xs"
                  :variant="state.socketStateFilter === item.value ? 'secondary' : 'ghost'"
                  @click="setSocketState(item.value)"
                >
                  {{ item.label }}
                </Button>
              </div>
            </div>
          </TooltipTrigger>
          <TooltipContent side="bottom" class="text-xs">
            套接字状态：全部 / 仅监听中 / 仅活跃连接
          </TooltipContent>
        </Tooltip>

        <Tooltip>
          <TooltipTrigger as-child>
            <div class="flex items-center gap-1.5">
              <span class="text-[11px] text-muted-foreground">协议</span>
              <div class="flex items-center rounded-md border p-0.5">
                <Button
                  v-for="item in socketProtocols"
                  :key="item.value"
                  size="xs"
                  :variant="state.socketProtocolFilter === item.value ? 'secondary' : 'ghost'"
                  @click="setSocketProtocol(item.value)"
                >
                  {{ item.label }}
                </Button>
              </div>
            </div>
          </TooltipTrigger>
          <TooltipContent side="bottom" class="text-xs">协议：全部 / 仅 TCP / 仅 UDP</TooltipContent>
        </Tooltip>
      </template>

      <div class="mx-0.5 hidden h-5 w-px bg-border sm:block" />

      <!-- 视图切换 -->
      <div class="flex items-center rounded-md border p-0.5">
        <Button
          v-for="item in views"
          :key="item.value"
          size="xs"
          :variant="state.view === item.value ? 'secondary' : 'ghost'"
          @click="setView(item.value)"
        >
          <component :is="item.icon" />
          {{ item.label }}
        </Button>
      </div>

      <template v-if="state.view === 'tree'">
        <Button size="icon-xs" variant="ghost" title="全部展开" @click="expandAll">
          <ChevronsUpDownIcon />
        </Button>
        <Button size="icon-xs" variant="ghost" title="全部折叠" @click="collapseAll">
          <ChevronsDownUpIcon />
        </Button>
      </template>

      <div class="ml-auto flex items-center gap-2">
        <span class="text-xs whitespace-nowrap text-muted-foreground">
          <template v-if="state.view === 'ports'">
            套接字
            <span class="font-mono tabular-nums text-foreground">
              {{ formatNumber(portsSummary.total) }}
            </span>
            / {{ formatNumber(socketCount) }}
          </template>
          <template v-else>
            <span class="font-mono tabular-nums text-foreground">
              {{ formatNumber(visibleCount) }}
            </span>
            / {{ formatNumber(processCount) }}
          </template>
        </span>
        <Button v-if="hasAnyFilter" size="xs" variant="ghost" @click="resetFilters">
          <FilterXIcon />
          清空过滤
        </Button>
      </div>
    </div>

    <!--
      次行：只承载「已生效的端口规则」回显，且仅在真有规则时才出现。
      所有筛选控件都在主行，这里不再放任何输入控件，避免出现「同类东西一半在上、一半在下」。
    -->
    <div
      v-if="rules.length > 0"
      class="flex flex-wrap items-center gap-2 border-t border-border/60 px-3 py-1.5"
    >
      <span class="text-[11px] text-muted-foreground">端口过滤</span>
      <Tooltip v-for="rule in rules" :key="rule.token">
        <TooltipTrigger as-child>
          <Badge
            variant="outline"
            class="h-5 cursor-pointer gap-1 border-sky-500/40 bg-sky-500/10 pr-1 font-mono text-[11px] text-sky-700 dark:text-sky-300"
            @click="removePortRule(rule.token)"
          >
            {{ ruleLabel(rule.token) }}
            <XIcon class="size-3 opacity-70" />
          </Badge>
        </TooltipTrigger>
        <TooltipContent side="bottom" class="text-xs">
          {{ ruleTooltip(rule.token) }} · 点击移除
        </TooltipContent>
      </Tooltip>

      <span class="text-[10px] text-muted-foreground/70">
        未加前缀的规则按「{{ scopeText }}」匹配
      </span>
    </div>
  </div>
</template>
