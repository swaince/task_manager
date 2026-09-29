<script setup lang="ts">
/**
 * 端口视图：每一条套接字一行（等价于 `netstat -ano` 加上进程归属与图标）。
 *
 * 与进程列表共享同一套端口过滤表达式，因此「按端口过滤」在这里是逐条套接字
 * 精确匹配，而不是按进程聚合后的近似匹配。
 */
import { computed } from 'vue'
import {
  ChevronDownIcon,
  CopyIcon,
  CrosshairIcon,
  FolderOpenIcon,
  Loader2Icon,
  SearchXIcon,
  SkullIcon,
  WaypointsIcon,
} from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import ProcessIcon from '@/components/process/ProcessIcon.vue'
import {
  copyText,
  filteredPortRows,
  focusWindow,
  isPortFiltered,
  portSummary,
  reveal,
  select,
  state,
  togglePortFilter,
  togglePortSort,
} from '@/composables/useProcessStore'
import type { PortRow, PortSortKey } from '@/types/process'

const emit = defineEmits<{
  (e: 'request-kill', pid: number, tree: boolean): void
}>()

interface Column {
  key: PortSortKey | 'address'
  label: string
  sortable: boolean
  class: string
}

const columns: Column[] = [
  { key: 'port', label: '端口', sortable: true, class: 'w-[92px]' },
  { key: 'protocol', label: '协议', sortable: true, class: 'w-[68px]' },
  { key: 'state', label: '状态', sortable: true, class: 'w-[124px]' },
  { key: 'address', label: '本地地址', sortable: false, class: 'min-w-[190px]' },
  { key: 'address', label: '远端地址', sortable: false, class: 'min-w-[180px]' },
  { key: 'process', label: '所属进程', sortable: true, class: 'min-w-[220px]' },
]

const rows = computed<PortRow[]>(() => filteredPortRows.value)
const summary = computed(() => portSummary.value)

function indicator(key: Column['key']): string {
  if (!key || !columns.some((column) => column.key === key && column.sortable)) return ''
  if (state.portSortKey !== key) return ''
  return state.portSortDirection === 'asc' ? '↑' : '↓'
}

function onHeaderClick(column: Column): void {
  if (!column.sortable) return
  togglePortSort(column.key as PortSortKey)
}

function remoteText(row: PortRow): string {
  const { remoteAddr, remotePort } = row.socket
  if (!remoteAddr || remotePort == null || remotePort === 0) return '—'
  return `${remoteAddr}:${remotePort}`
}

function stateClass(row: PortRow): string {
  if (row.socket.listening) return 'text-emerald-600 dark:text-emerald-400'
  if (row.socket.state === 'ESTABLISHED') return 'text-sky-600 dark:text-sky-400'
  if (row.socket.state === 'TIME_WAIT' || row.socket.state === 'CLOSE_WAIT') {
    return 'text-amber-600 dark:text-amber-400'
  }
  return 'text-muted-foreground'
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col">
    <!-- 汇总条 -->
    <div class="flex shrink-0 flex-wrap items-center gap-3 border-b bg-card/30 px-3 py-1.5 text-[11px]">
      <span class="flex items-center gap-1.5 font-medium">
        <WaypointsIcon class="size-3.5 text-muted-foreground" />
        套接字
        <span class="font-mono tabular-nums">{{ summary.total }}</span>
      </span>
      <span class="text-muted-foreground">
        监听
        <span class="font-mono tabular-nums text-emerald-600 dark:text-emerald-400">
          {{ summary.listening }}
        </span>
      </span>
      <span class="text-muted-foreground">
        活跃连接
        <span class="font-mono tabular-nums text-sky-600 dark:text-sky-400">
          {{ summary.active }}
        </span>
      </span>
      <span class="text-muted-foreground">
        涉及进程
        <span class="font-mono tabular-nums">{{ summary.processes }}</span>
      </span>
      <span class="ml-auto text-muted-foreground/70">
        点击行内端口数字可直接增删该端口的过滤条件
      </span>
    </div>

    <div class="relative min-h-0 flex-1 overflow-auto outline-none">
      <table class="w-full border-separate border-spacing-0 text-xs">
        <thead class="sticky top-0 z-20">
          <tr>
            <th
              v-for="(column, index) in columns"
              :key="`${column.label}-${index}`"
              class="border-b bg-card/95 px-2 py-2 text-left font-medium whitespace-nowrap text-muted-foreground backdrop-blur select-none"
              :class="[
                column.class,
                column.sortable ? 'cursor-pointer hover:text-foreground' : '',
                state.portSortKey === column.key ? 'text-foreground' : '',
              ]"
              @click="onHeaderClick(column)"
            >
              {{ column.label }}
              <span class="ml-0.5 font-mono text-[10px]">{{ indicator(column.key) }}</span>
            </th>
            <th class="w-[44px] border-b bg-card/95 px-1 py-2 backdrop-blur" />
          </tr>
        </thead>

        <tbody>
          <tr v-if="rows.length === 0">
            <td :colspan="columns.length + 1" class="py-16">
              <div class="flex flex-col items-center gap-2 text-muted-foreground">
                <Loader2Icon v-if="state.loading" class="size-6 animate-spin" />
                <SearchXIcon v-else class="size-6" />
                <p class="text-sm">
                  {{ state.loading ? '正在采集套接字信息…' : '没有符合当前端口过滤条件的套接字' }}
                </p>
                <p class="max-w-md text-center text-xs leading-relaxed">
                  端口输入框支持
                  <code class="font-mono">8080</code>、
                  <code class="font-mono">80,443</code>、
                  <code class="font-mono">8000-8100</code>、
                  <code class="font-mono">tcp:8080</code>、
                  <code class="font-mono">remote:443</code>
                </p>
              </div>
            </td>
          </tr>

          <tr
            v-for="row in rows"
            :key="row.key"
            class="group cursor-default transition-colors"
            :class="
              state.selectedPid === row.pid
                ? 'bg-primary/10'
                : 'hover:bg-muted/40 odd:bg-muted/[0.12]'
            "
            @click="select(row.pid)"
          >
            <!-- 端口：点击即切换该端口的过滤 -->
            <td class="border-b border-border/50 px-2 py-1.5">
              <button
                type="button"
                class="rounded-sm px-1 font-mono text-[12px] tabular-nums transition-colors"
                :class="
                  isPortFiltered(row.socket.localPort)
                    ? 'bg-sky-500/20 text-sky-700 dark:text-sky-300'
                    : row.socket.listening
                      ? 'text-emerald-600 hover:bg-emerald-500/10 dark:text-emerald-400'
                      : 'hover:bg-muted'
                "
                :title="`点击按端口 ${row.socket.localPort} 过滤`"
                @click.stop="togglePortFilter(row.socket.localPort, row.socket.protocol)"
              >
                {{ row.socket.localPort }}
              </button>
            </td>

            <!-- 协议 -->
            <td class="border-b border-border/50 px-2 py-1.5">
              <Badge variant="outline" class="h-4 px-1 font-mono text-[10px]">
                {{ row.socket.protocol }}
              </Badge>
            </td>

            <!-- 状态 -->
            <td class="border-b border-border/50 px-2 py-1.5">
              <span class="flex items-center gap-1.5">
                <span
                  v-if="row.socket.listening"
                  class="size-1.5 shrink-0 rounded-full bg-emerald-500"
                  aria-hidden="true"
                />
                <span :class="stateClass(row)" class="font-mono text-[11px]">
                  {{ row.socket.state }}
                </span>
              </span>
            </td>

            <!-- 本地地址 -->
            <td class="border-b border-border/50 px-2 py-1.5">
              <button
                type="button"
                class="rounded-sm font-mono text-[11px] text-muted-foreground hover:bg-muted hover:text-foreground"
                :title="`点击按端口 ${row.socket.localPort} 过滤`"
                @click.stop="togglePortFilter(row.socket.localPort, row.socket.protocol)"
              >
                {{ row.socket.localAddr }}:{{ row.socket.localPort }}
              </button>
            </td>

            <!-- 远端地址：端口同样可点击，自动带 remote: 前缀 -->
            <td class="border-b border-border/50 px-2 py-1.5">
              <span v-if="row.socket.remotePort && row.socket.remoteAddr" class="font-mono text-[11px]">
                <span class="text-muted-foreground">{{ row.socket.remoteAddr }}:</span>
                <button
                  type="button"
                  class="rounded-sm px-1 transition-colors"
                  :class="
                    isPortFiltered(row.socket.remotePort)
                      ? 'bg-sky-500/20 text-sky-700 dark:text-sky-300'
                      : 'text-muted-foreground hover:bg-muted hover:text-foreground'
                  "
                  :title="`点击按远程端口 ${row.socket.remotePort} 过滤`"
                  @click.stop="
                    togglePortFilter(row.socket.remotePort!, row.socket.protocol, 'remote')
                  "
                >
                  {{ row.socket.remotePort }}
                </button>
              </span>
              <span v-else class="font-mono text-[11px] text-muted-foreground">—</span>
            </td>

            <!-- 所属进程 -->
            <td class="border-b border-border/50 px-2 py-1.5">
              <div class="flex items-center gap-2">
                <ProcessIcon :exe="row.exe" :name="row.name" :size="16" />
                <span class="truncate font-medium">{{ row.displayName }}</span>
                <span class="shrink-0 font-mono text-[10px] text-muted-foreground">
                  {{ row.pid }}
                </span>
              </div>
            </td>

            <!-- 行操作 -->
            <td class="border-b border-border/50 px-1 py-1.5 text-right">
              <DropdownMenu>
                <DropdownMenuTrigger as-child>
                  <Button
                    size="icon-xs"
                    variant="ghost"
                    class="opacity-0 group-hover:opacity-100 data-[state=open]:opacity-100"
                    @click.stop
                  >
                    ⋯
                  </Button>
                </DropdownMenuTrigger>
                <DropdownMenuContent align="end" class="w-56">
                  <DropdownMenuLabel class="flex items-center justify-between gap-2">
                    <span class="truncate">{{ row.displayName }}</span>
                    <span class="font-mono text-[10px] text-muted-foreground">{{ row.pid }}</span>
                  </DropdownMenuLabel>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem @click.stop="select(row.pid)">
                    <ChevronDownIcon />
                    查看进程详情
                  </DropdownMenuItem>
                  <DropdownMenuItem @click.stop="focusWindow(row.pid)">
                    <CrosshairIcon />
                    切换到前台窗口
                  </DropdownMenuItem>
                  <DropdownMenuItem :disabled="!row.exe" @click.stop="reveal(row.exe)">
                    <FolderOpenIcon />
                    打开文件位置
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    @click.stop="
                      copyText(
                        `${row.socket.protocol} ${row.socket.localAddr}:${row.socket.localPort} -> ${remoteText(row)} [${row.socket.state}] ${row.displayName}(${row.pid})`,
                        '端口信息',
                      )
                    "
                  >
                    <CopyIcon />
                    复制这一行
                  </DropdownMenuItem>
                  <DropdownMenuItem
                    @click.stop="copyText(String(row.pid), 'PID')"
                  >
                    <CopyIcon />
                    复制 PID
                  </DropdownMenuItem>
                  <DropdownMenuSeparator />
                  <DropdownMenuItem variant="destructive" @click.stop="emit('request-kill', row.pid, false)">
                    <SkullIcon />
                    结束进程
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
