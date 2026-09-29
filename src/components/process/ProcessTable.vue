<script setup lang="ts">
/**
 * 进程数据网格：同一份实现同时支撑「平铺列表」与「进程树」两种视图。
 *
 * * 列表模式：渲染 `visibleProcesses`（已过滤 + 已排序）。
 * * 树模式：渲染 `treeRows`（父子关系展平，按深度缩进）。
 */
import { computed } from 'vue'
import {
  ChevronDownIcon,
  ChevronRightIcon,
  CopyIcon,
  CrosshairIcon,
  FolderOpenIcon,
  Loader2Icon,
  SearchXIcon,
  SkullIcon,
  Trash2Icon,
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
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import ProcessIcon from '@/components/process/ProcessIcon.vue'
import PortBadge from '@/components/process/PortBadge.vue'
import {
  copyText,
  focusWindow,
  isExpanded,
  isMatched,
  reveal,
  select,
  state,
  toggleExpand,
  toggleSort,
  treeRows,
  visibleProcesses,
} from '@/composables/useProcessStore'
import {
  formatBytes,
  formatDateTime,
  formatPercent,
  statusLabel,
  truncateMiddle,
} from '@/lib/format'
import type { ProcessInfo, SortKey } from '@/types/process'

const emit = defineEmits<{
  (e: 'request-kill', pid: number, tree: boolean): void
}>()

interface Column {
  key: SortKey | 'cmdline'
  label: string
  sortable: boolean
  class: string
}

const columns: Column[] = [
  { key: 'name', label: '进程', sortable: true, class: 'min-w-[240px]' },
  { key: 'status', label: '状态', sortable: true, class: 'w-[86px]' },
  { key: 'cpu', label: 'CPU', sortable: true, class: 'w-[78px] text-right' },
  { key: 'memory', label: '内存', sortable: true, class: 'w-[92px] text-right' },
  { key: 'threads', label: '线程', sortable: true, class: 'w-[64px] text-right' },
  { key: 'ports', label: '端口', sortable: true, class: 'w-[168px]' },
  { key: 'user', label: '用户', sortable: true, class: 'w-[128px]' },
  { key: 'cmdline', label: '命令行', sortable: false, class: 'min-w-[220px]' },
]

/** 当前视图的数据行，统一成 `{ process, depth }` 形状，便于共用模板。 */
interface Row {
  process: ProcessInfo
  depth: number
  hasChildren: boolean
}

const rows = computed<Row[]>(() => {
  if (state.view === 'tree') {
    return treeRows.value.map((node) => ({
      process: node.process,
      depth: node.depth,
      hasChildren: node.hasChildren,
    }))
  }
  return visibleProcesses.value.map((process) => ({ process, depth: 0, hasChildren: false }))
})

const treeMode = computed(() => state.view === 'tree')

function sortIndicator(key: Column['key']): string {
  if (!key || !columns.some((c) => c.key === key && c.sortable)) return ''
  if (state.sortKey !== key) return ''
  return state.sortDirection === 'asc' ? '↑' : '↓'
}

function onHeaderClick(column: Column): void {
  if (!column.sortable) return
  toggleSort(column.key as SortKey)
}

function onRowClick(process: ProcessInfo): void {
  void select(process.pid)
}

function onRowDoubleClick(process: ProcessInfo): void {
  if (process.windowTitles.length > 0) void focusWindow(process.pid)
}

/** 键盘导航：↑/↓ 移动选中，←/→ 折叠展开，Delete 触发结束确认。 */
function onKeydown(event: KeyboardEvent): void {
  const list = rows.value
  if (list.length === 0) return
  const index = list.findIndex((row) => row.process.pid === state.selectedPid)

  const move = (delta: number): void => {
    const next = index < 0 ? 0 : Math.min(list.length - 1, Math.max(0, index + delta))
    const pid = list[next].process.pid
    void select(pid)
    requestAnimationFrame(() => {
      document
        .querySelector(`[data-pid="${pid}"]`)
        ?.scrollIntoView({ block: 'nearest', behavior: 'smooth' })
    })
  }

  switch (event.key) {
    case 'ArrowDown':
      event.preventDefault()
      move(1)
      break
    case 'ArrowUp':
      event.preventDefault()
      move(-1)
      break
    case 'ArrowRight':
      if (treeMode.value && index >= 0 && list[index].hasChildren) toggleExpand(list[index].process.pid)
      break
    case 'ArrowLeft':
      if (treeMode.value && index >= 0 && list[index].hasChildren) toggleExpand(list[index].process.pid)
      break
    case 'Enter':
      if (index >= 0) onRowDoubleClick(list[index].process)
      break
    case 'Delete':
      if (index >= 0) emit('request-kill', list[index].process.pid, false)
      break
    default:
      break
  }
}

function cpuClass(process: ProcessInfo): string {
  if (process.cpuUsage >= 50) return 'text-rose-500'
  if (process.cpuUsage >= 10) return 'text-amber-500'
  return 'text-foreground'
}

function dimmed(process: ProcessInfo): boolean {
  if (!treeMode.value) return false
  if (!state.query.trim() && !state.portQuery.trim() && !state.onlyListening && !state.onlyWindowed) {
    return false
  }
  return !isMatched(process.pid)
}
</script>

<template>
  <div ref="scrollHost" class="relative h-full min-h-0 overflow-auto outline-none" tabindex="0" @keydown="onKeydown">
    <table class="w-full border-separate border-spacing-0 text-xs">
      <thead class="sticky top-0 z-20">
        <tr>
          <th
            v-for="column in columns"
            :key="column.key"
            class="border-b bg-card/95 px-2 py-2 text-left font-medium whitespace-nowrap text-muted-foreground backdrop-blur select-none"
            :class="[
              column.class,
              column.sortable ? 'cursor-pointer hover:text-foreground' : '',
              state.sortKey === column.key ? 'text-foreground' : '',
            ]"
            @click="onHeaderClick(column)"
          >
            {{ column.label }}
            <span class="ml-0.5 font-mono text-[10px]">{{ sortIndicator(column.key) }}</span>
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
                {{ state.loading ? '正在采集进程信息…' : '没有符合当前过滤条件的进程' }}
              </p>
              <p v-if="state.error" class="max-w-md text-center font-mono text-xs text-rose-500">
                {{ state.error }}
              </p>
            </div>
          </td>
        </tr>

        <tr
          v-for="row in rows"
          :key="row.process.pid"
          :data-pid="row.process.pid"
          class="group cursor-default transition-colors"
          :class="[
            state.selectedPid === row.process.pid
              ? 'bg-primary/10'
              : 'hover:bg-muted/40 odd:bg-muted/[0.12]',
            dimmed(row.process) ? 'opacity-45' : '',
          ]"
          @click="onRowClick(row.process)"
          @dblclick="onRowDoubleClick(row.process)"
        >
          <!-- 进程名 -->
          <td class="border-b border-border/50 px-2 py-1.5">
            <div
              class="flex items-center gap-2"
              :style="{ paddingLeft: treeMode ? `${row.depth * 14}px` : '0px' }"
            >
              <button
                v-if="treeMode && row.hasChildren"
                class="flex size-4 shrink-0 items-center justify-center rounded text-muted-foreground hover:bg-muted hover:text-foreground"
                type="button"
                @click.stop="toggleExpand(row.process.pid)"
              >
                <ChevronDownIcon v-if="isExpanded(row.process.pid)" class="size-3.5" />
                <ChevronRightIcon v-else class="size-3.5" />
              </button>
              <span v-else-if="treeMode" class="size-4 shrink-0" />

              <ProcessIcon :exe="row.process.exe" :name="row.process.name" :size="16" />

              <div class="min-w-0">
                <div class="flex items-center gap-1.5">
                  <span class="truncate font-medium text-foreground">{{ row.process.displayName }}</span>
                  <span class="shrink-0 font-mono text-[10px] text-muted-foreground">
                    {{ row.process.pid }}
                  </span>
                  <Tooltip v-if="!row.process.accessible">
                    <TooltipTrigger as-child>
                      <Badge
                        variant="outline"
                        class="h-4 border-amber-500/40 px-1 text-[10px] text-amber-600 dark:text-amber-400"
                      >
                        受保护
                      </Badge>
                    </TooltipTrigger>
                    <TooltipContent side="top" class="max-w-xs text-xs">
                      无法读取该进程的命令行与工作目录，通常需要管理员权限。
                    </TooltipContent>
                  </Tooltip>
                </div>
                <div
                  v-if="row.process.windowTitles.length > 0"
                  class="truncate text-[11px] text-muted-foreground"
                >
                  {{ row.process.windowTitles[0] }}
                </div>
              </div>
            </div>
          </td>

          <!-- 状态 -->
          <td class="border-b border-border/50 px-2 py-1.5">
            <Badge
              variant="outline"
              class="h-5 px-1.5 text-[10px]"
              :class="row.process.status === 'Run' ? 'border-emerald-500/40 text-emerald-600 dark:text-emerald-400' : ''"
            >
              {{ statusLabel(row.process.status) }}
            </Badge>
          </td>

          <!-- CPU -->
          <td class="border-b border-border/50 px-2 py-1.5 text-right font-mono tabular-nums">
            <span :class="cpuClass(row.process)">
              {{ row.process.cpuUsage > 0 ? formatPercent(row.process.cpuUsage) : '—' }}
            </span>
          </td>

          <!-- 内存 -->
          <td class="border-b border-border/50 px-2 py-1.5 text-right font-mono tabular-nums">
            {{ formatBytes(row.process.memory) }}
          </td>

          <!-- 线程 / 句柄 -->
          <td class="border-b border-border/50 px-2 py-1.5 text-right font-mono tabular-nums text-muted-foreground">
            <Tooltip>
              <TooltipTrigger as-child>
                <span>
                  {{ row.process.threadCount }}
                  <span v-if="row.process.handleCount != null" class="text-muted-foreground/60">
                    / {{ row.process.handleCount }}
                  </span>
                </span>
              </TooltipTrigger>
              <TooltipContent side="top" class="text-xs">
                线程数{{ row.process.handleCount != null ? ' / 句柄数' : '' }}
              </TooltipContent>
            </Tooltip>
          </td>

          <!-- 端口 -->
          <td class="border-b border-border/50 px-2 py-1.5">
            <PortBadge
              :ports="row.process.ports"
              :listening-ports="row.process.listeningPorts"
              :limit="2"
            />
          </td>

          <!-- 用户 -->
          <td class="border-b border-border/50 px-2 py-1.5">
            <span class="block truncate text-muted-foreground" :title="row.process.user ?? ''">
              {{ row.process.user ?? '—' }}
            </span>
          </td>

          <!-- 命令行 -->
          <td class="border-b border-border/50 px-2 py-1.5">
            <Tooltip>
              <TooltipTrigger as-child>
                <span
                  class="block truncate font-mono text-[11px] text-muted-foreground"
                  :title="row.process.cmdLine"
                >
                  {{ row.process.cmdLine ? truncateMiddle(row.process.cmdLine, 160) : '—' }}
                </span>
              </TooltipTrigger>
              <TooltipContent
                v-if="row.process.cmdLine"
                side="bottom"
                align="start"
                class="max-w-2xl font-mono text-[11px] break-all"
              >
                {{ row.process.cmdLine }}
              </TooltipContent>
            </Tooltip>
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
              <DropdownMenuContent align="end" class="w-52">
                <DropdownMenuLabel class="flex items-center justify-between gap-2">
                  <span class="truncate">{{ row.process.displayName }}</span>
                  <span class="font-mono text-[10px] text-muted-foreground">{{ row.process.pid }}</span>
                </DropdownMenuLabel>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  :disabled="row.process.windowTitles.length === 0"
                  @select="focusWindow(row.process.pid)"
                >
                  <CrosshairIcon />
                  切换到前台窗口
                </DropdownMenuItem>
                <DropdownMenuItem
                  :disabled="!row.process.exe"
                  @select="reveal(row.process.exe)"
                >
                  <FolderOpenIcon />
                  打开文件位置
                </DropdownMenuItem>
                <DropdownMenuItem @select="copyText(String(row.process.pid), 'PID')">
                  <CopyIcon />
                  复制 PID
                </DropdownMenuItem>
                <DropdownMenuItem
                  :disabled="!row.process.cmdLine"
                  @select="copyText(row.process.cmdLine, '命令行')"
                >
                  <CopyIcon />
                  复制命令行
                </DropdownMenuItem>
                <DropdownMenuItem
                  @select="
                    copyText(
                      [
                        row.process.displayName,
                        `PID=${row.process.pid}`,
                        `PPID=${row.process.parentPid ?? '-'}`,
                        `EXE=${row.process.exe ?? '-'}`,
                        `USER=${row.process.user ?? '-'}`,
                        `MEM=${formatBytes(row.process.memory)}`,
                        `START=${formatDateTime(row.process.startTime)}`,
                        `CMD=${row.process.cmdLine || '-'}`,
                      ].join('\n'),
                      '进程信息',
                    )
                  "
                >
                  <CopyIcon />
                  复制进程信息
                </DropdownMenuItem>
                <DropdownMenuSeparator />
                <DropdownMenuItem
                  variant="destructive"
                  @select="emit('request-kill', row.process.pid, false)"
                >
                  <SkullIcon />
                  结束进程
                </DropdownMenuItem>
                <DropdownMenuItem
                  variant="destructive"
                  :disabled="row.process.children.length === 0"
                  @select="emit('request-kill', row.process.pid, true)"
                >
                  <Trash2Icon />
                  结束进程树（{{ row.process.children.length + 1 }}）
                </DropdownMenuItem>
              </DropdownMenuContent>
            </DropdownMenu>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
