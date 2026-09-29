<script setup lang="ts">
/**
 * 右侧详情面板：选中进程后的「全貌」。
 *
 * 数据按需从后端拉取（`get_process_detail`），因为环境变量这类字段
 * 每次全量采样都读一遍代价太高，只在用户真正点开时才取。
 */
import { computed, ref, watch } from 'vue'
import {
  CopyIcon,
  CrosshairIcon,
  FolderOpenIcon,
  HashIcon,
  InfoIcon,
  LayersIcon,
  SearchIcon,
  ServerIcon,
  SkullIcon,
  SquareTerminalIcon,
  Trash2Icon,
  UserIcon,
  WaypointsIcon,
} from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Separator } from '@/components/ui/separator'
import { Skeleton } from '@/components/ui/skeleton'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import ProcessIcon from '@/components/process/ProcessIcon.vue'
import {
  copyText,
  detail,
  detailError,
  detailLoading,
  focusWindow,
  isPortFiltered,
  reveal,
  select,
  state,
  togglePortFilter,
} from '@/composables/useProcessStore'
import {
  formatBytes,
  formatDateTime,
  formatDuration,
  formatNumber,
  formatPercent,
  statusLabel,
} from '@/lib/format'
import type { ProcessInfo } from '@/types/process'

const emit = defineEmits<{
  (e: 'request-kill', pid: number, tree: boolean): void
}>()

const tab = ref('overview')
const envFilter = ref('')
const socketFilter = ref('')

watch(
  () => state.selectedPid,
  () => {
    tab.value = 'overview'
    envFilter.value = ''
    socketFilter.value = ''
  },
)

const process = computed<ProcessInfo | null>(() => detail.value?.process ?? null)

/** 该进程的全部套接字。 */
const allSockets = computed(() => detail.value?.sockets ?? process.value?.sockets ?? [])

/** 端口页签内的本地过滤：按端口号 / 地址 / 状态 / 协议筛。 */
const sockets = computed(() => {
  const keyword = socketFilter.value.trim().toLowerCase()
  if (!keyword) return allSockets.value
  return allSockets.value.filter(
    (socket) =>
      String(socket.localPort).includes(keyword) ||
      (socket.remotePort != null && String(socket.remotePort).includes(keyword)) ||
      socket.state.toLowerCase().includes(keyword) ||
      socket.localAddr.toLowerCase().includes(keyword) ||
      socket.protocol.toLowerCase().includes(keyword),
  )
})

const socketStats = computed(() => {
  let listening = 0
  for (const socket of allSockets.value) if (socket.listening) listening += 1
  return {
    total: allSockets.value.length,
    listening,
    active: allSockets.value.length - listening,
  }
})

const environment = computed(() => {
  const list = detail.value?.environment ?? []
  const keyword = envFilter.value.trim().toLowerCase()
  if (!keyword) return list
  return list.filter(
    (item) =>
      item.key.toLowerCase().includes(keyword) || item.value.toLowerCase().includes(keyword),
  )
})

const listeningSummary = computed(() => {
  const ports = process.value?.listeningPorts ?? []
  return ports.length > 0 ? ports.join(', ') : '—'
})

const ancestorChain = computed(() => [...(detail.value?.ancestors ?? [])].reverse())

function copyAllEnvironment(): void {
  const text = (detail.value?.environment ?? [])
    .map((item) => `${item.key}=${item.value}`)
    .join('\r\n')
  void copyText(text, '环境变量')
}

function copyInfo(): void {
  const p = process.value
  if (!p) return
  const lines = [
    `名称: ${p.displayName} (${p.name})`,
    `PID: ${p.pid}`,
    `父进程: ${p.parentPid ?? '-'}`,
    `路径: ${p.exe ?? '-'}`,
    `工作目录: ${p.cwd ?? '-'}`,
    `用户: ${p.user ?? '-'}`,
    `状态: ${statusLabel(p.status)}`,
    `启动时间: ${formatDateTime(p.startTime)}`,
    `运行时长: ${formatDuration(p.runTime)}`,
    `CPU: ${formatPercent(p.cpuUsage)}`,
    `内存: ${formatBytes(p.memory)}`,
    `虚拟内存: ${formatBytes(p.virtualMemory)}`,
    `线程: ${p.threadCount}`,
    `句柄: ${p.handleCount ?? '-'}`,
    `监听端口: ${p.listeningPorts.join(', ') || '-'}`,
    `全部端口: ${p.ports.join(', ') || '-'}`,
    `窗口: ${p.windowTitles.join(' | ') || '-'}`,
    `命令行: ${p.cmdLine || '-'}`,
  ]
  void copyText(lines.join('\r\n'), '进程详情')
}
</script>

<template>
  <div class="flex h-full min-h-0 flex-col bg-card/20">
    <!-- 空状态 -->
    <div
      v-if="state.selectedPid == null"
      class="flex flex-1 flex-col items-center justify-center gap-3 px-8 text-center text-muted-foreground"
    >
      <div class="rounded-full border border-dashed p-4">
        <InfoIcon class="size-6" />
      </div>
      <p class="text-sm font-medium text-foreground">未选择进程</p>
      <p class="max-w-[240px] text-xs leading-relaxed">
        在左侧列表中单击任意进程查看详细信息：命令行、端口、环境变量与父子关系。
      </p>
      <div class="mt-1 flex flex-wrap justify-center gap-1 text-[11px]">
        <Badge variant="outline">↑ ↓ 切换</Badge>
        <Badge variant="outline">Enter 定位窗口</Badge>
        <Badge variant="outline">Delete 结束进程</Badge>
      </div>
    </div>

    <template v-else>
      <!-- 标题 -->
      <div class="shrink-0 border-b px-3 py-2.5">
        <div v-if="detailLoading && !process" class="space-y-2">
          <Skeleton class="h-5 w-40" />
          <Skeleton class="h-3 w-64" />
        </div>
        <template v-else-if="process">
          <div class="flex items-start gap-2">
            <ProcessIcon :exe="process.exe" :name="process.name" :size="28" class="mt-0.5" />
            <div class="min-w-0 flex-1">
              <div class="flex items-center gap-2">
                <h2 class="truncate text-sm font-semibold" :title="process.exe ?? process.name">
                  {{ process.displayName }}
                </h2>
                <Badge variant="outline" class="font-mono text-[10px]">PID {{ process.pid }}</Badge>
                <Badge
                  v-if="!process.accessible"
                  variant="outline"
                  class="border-amber-500/40 text-[10px] text-amber-600 dark:text-amber-400"
                >
                  受保护
                </Badge>
              </div>
              <p class="mt-0.5 truncate font-mono text-[11px] text-muted-foreground" :title="process.exe ?? ''">
                {{ process.exe ?? '（无法读取可执行文件路径）' }}
              </p>
              <p
                v-if="process.windowTitles.length > 0"
                class="mt-0.5 truncate text-[11px] text-sky-600 dark:text-sky-400"
              >
                {{ process.windowTitles[0] }}
              </p>
            </div>
          </div>
        </template>
      </div>

      <!-- 操作 -->
      <div class="flex shrink-0 flex-wrap items-center gap-1.5 border-b px-3 py-2">
        <Button
          size="xs"
          variant="outline"
          :disabled="!process || process.windowTitles.length === 0"
          @click="process && focusWindow(process.pid)"
        >
          <CrosshairIcon />
          定位窗口
        </Button>
        <Button
          size="xs"
          variant="outline"
          :disabled="!process?.exe"
          @click="reveal(process?.exe ?? null)"
        >
          <FolderOpenIcon />
          打开位置
        </Button>
        <Button size="xs" variant="outline" :disabled="!process" @click="copyInfo">
          <CopyIcon />
          复制详情
        </Button>
        <Button
          size="xs"
          variant="outline"
          :disabled="!process || process.children.length === 0"
          @click="emit('request-kill', process!.pid, true)"
        >
          <Trash2Icon />
          结束进程树
        </Button>
        <Button
          size="xs"
          variant="destructive"
          class="ml-auto"
          :disabled="!process"
          @click="emit('request-kill', process!.pid, false)"
        >
          <SkullIcon />
          结束进程
        </Button>
      </div>

      <p v-if="detailError" class="shrink-0 border-b bg-destructive/10 px-3 py-1.5 text-xs text-destructive">
        {{ detailError }}
      </p>

      <Tabs v-model="tab" class="flex min-h-0 flex-1 flex-col gap-0">
        <TabsList variant="line" class="mx-3 mt-2 w-fit shrink-0">
          <TabsTrigger value="overview">概览</TabsTrigger>
          <TabsTrigger value="ports">端口 {{ socketStats.total }}</TabsTrigger>
          <TabsTrigger value="cmdline">命令行</TabsTrigger>
          <TabsTrigger value="env">环境变量 {{ detail?.environment.length ?? 0 }}</TabsTrigger>
          <TabsTrigger value="relations">关系</TabsTrigger>
        </TabsList>

        <ScrollArea class="min-h-0 flex-1">
          <!-- 概览 -->
          <TabsContent value="overview" class="mt-0 space-y-3 p-3">
            <div class="grid grid-cols-2 gap-2">
              <div class="rounded-md border bg-background/60 px-2.5 py-2">
                <div class="flex items-center gap-1 text-[11px] text-muted-foreground">
                  <HashIcon class="size-3" /> PID / 父进程
                </div>
                <div class="mt-0.5 font-mono text-sm">
                  {{ process?.pid ?? '-' }} / {{ process?.parentPid ?? '-' }}
                </div>
              </div>
              <div class="rounded-md border bg-background/60 px-2.5 py-2">
                <div class="flex items-center gap-1 text-[11px] text-muted-foreground">
                  <UserIcon class="size-3" /> 用户
                </div>
                <div class="mt-0.5 truncate text-sm">{{ process?.user ?? '-' }}</div>
              </div>
              <div class="rounded-md border bg-background/60 px-2.5 py-2">
                <div class="text-[11px] text-muted-foreground">CPU / 内存</div>
                <div class="mt-0.5 font-mono text-sm tabular-nums">
                  {{ formatPercent(process?.cpuUsage ?? 0) }} ·
                  {{ formatBytes(process?.memory ?? 0) }}
                </div>
              </div>
              <div class="rounded-md border bg-background/60 px-2.5 py-2">
                <div class="text-[11px] text-muted-foreground">线程 / 句柄</div>
                <div class="mt-0.5 font-mono text-sm tabular-nums">
                  {{ process?.threadCount ?? '-' }} / {{ process?.handleCount ?? '-' }}
                </div>
              </div>
              <div class="rounded-md border bg-background/60 px-2.5 py-2">
                <div class="text-[11px] text-muted-foreground">虚拟内存</div>
                <div class="mt-0.5 font-mono text-sm tabular-nums">
                  {{ formatBytes(process?.virtualMemory ?? 0) }}
                </div>
              </div>
              <div class="rounded-md border bg-background/60 px-2.5 py-2">
                <div class="text-[11px] text-muted-foreground">状态</div>
                <div class="mt-0.5 text-sm">{{ statusLabel(process?.status ?? '') }}</div>
              </div>
            </div>

            <Separator />

            <dl class="space-y-2 text-xs">
              <div class="grid grid-cols-[88px_1fr] gap-2">
                <dt class="text-muted-foreground">启动时间</dt>
                <dd class="font-mono">{{ formatDateTime(process?.startTime ?? 0) }}</dd>
              </div>
              <div class="grid grid-cols-[88px_1fr] gap-2">
                <dt class="text-muted-foreground">运行时长</dt>
                <dd class="font-mono">{{ formatDuration(process?.runTime ?? 0) }}</dd>
              </div>
              <div class="grid grid-cols-[88px_1fr] gap-2">
                <dt class="text-muted-foreground">工作目录</dt>
                <dd class="font-mono break-all">{{ process?.cwd ?? '—' }}</dd>
              </div>
              <div class="grid grid-cols-[88px_1fr] gap-2">
                <dt class="text-muted-foreground">监听端口</dt>
                <dd class="font-mono text-emerald-600 dark:text-emerald-400">
                  {{ listeningSummary }}
                </dd>
              </div>
              <div class="grid grid-cols-[88px_1fr] gap-2">
                <dt class="text-muted-foreground">全部端口</dt>
                <dd class="font-mono break-all">
                  {{ process?.ports.length ? process.ports.join(', ') : '—' }}
                </dd>
              </div>
              <div class="grid grid-cols-[88px_1fr] gap-2">
                <dt class="text-muted-foreground">子进程</dt>
                <dd class="font-mono">{{ formatNumber(process?.children.length ?? 0) }} 个</dd>
              </div>
            </dl>
          </TabsContent>

          <!-- 端口 -->
          <TabsContent value="ports" class="mt-0 space-y-2 p-3">
            <div class="flex items-center gap-2">
              <div class="relative flex-1">
                <SearchIcon
                  class="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground"
                />
                <Input
                  v-model="socketFilter"
                  class="h-7 pl-7 font-mono text-xs"
                  placeholder="过滤套接字：端口 / 地址 / 状态"
                  spellcheck="false"
                />
              </div>
              <div class="flex shrink-0 items-center gap-1.5 text-[11px] text-muted-foreground">
                <Badge variant="outline" class="h-5 font-mono text-[10px]">
                  共 {{ socketStats.total }}
                </Badge>
                <Badge
                  variant="outline"
                  class="h-5 border-emerald-500/40 font-mono text-[10px] text-emerald-600 dark:text-emerald-400"
                >
                  监听 {{ socketStats.listening }}
                </Badge>
                <Badge variant="outline" class="h-5 font-mono text-[10px]">
                  活跃 {{ socketStats.active }}
                </Badge>
              </div>
            </div>

            <div
              v-if="allSockets.length === 0"
              class="rounded-md border border-dashed py-10 text-center text-xs text-muted-foreground"
            >
              该进程当前没有网络套接字
            </div>
            <div
              v-else-if="sockets.length === 0"
              class="rounded-md border border-dashed py-10 text-center text-xs text-muted-foreground"
            >
              没有匹配「{{ socketFilter }}」的套接字
            </div>
            <table v-else class="w-full text-[11px]">
              <thead>
                <tr class="text-left text-muted-foreground">
                  <th class="pb-1.5 font-medium">协议</th>
                  <th class="pb-1.5 font-medium">本地地址</th>
                  <th class="pb-1.5 font-medium">远端地址</th>
                  <th class="pb-1.5 font-medium">状态</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="(socket, index) in sockets"
                  :key="`${socket.protocol}-${socket.localPort}-${socket.remotePort}-${index}`"
                  class="border-t border-border/50"
                >
                  <td class="py-1.5">
                    <Badge variant="outline" class="h-4 px-1 font-mono text-[10px]">
                      {{ socket.protocol }}
                    </Badge>
                  </td>
                  <td class="py-1.5">
                    <button
                      type="button"
                      class="rounded-sm px-1 font-mono transition-colors"
                      :class="
                        isPortFiltered(socket.localPort)
                          ? 'bg-sky-500/20 text-sky-700 dark:text-sky-300'
                          : 'hover:bg-muted'
                      "
                      :title="`按端口 ${socket.localPort} 过滤全部进程`"
                      @click="togglePortFilter(socket.localPort, socket.protocol)"
                    >
                      {{ socket.localAddr }}:{{ socket.localPort }}
                    </button>
                  </td>
                  <td class="py-1.5">
                    <span
                      v-if="socket.remoteAddr && socket.remotePort != null && socket.remotePort > 0"
                      class="font-mono text-muted-foreground"
                    >
                      {{ socket.remoteAddr }}:
                      <button
                        type="button"
                        class="rounded-sm px-1 transition-colors"
                        :class="
                          isPortFiltered(socket.remotePort)
                            ? 'bg-sky-500/20 text-sky-700 dark:text-sky-300'
                            : 'hover:bg-muted hover:text-foreground'
                        "
                        :title="`按远程端口 ${socket.remotePort} 过滤全部进程`"
                        @click="togglePortFilter(socket.remotePort!, socket.protocol, 'remote')"
                      >
                        {{ socket.remotePort }}
                      </button>
                    </span>
                    <span v-else class="font-mono text-muted-foreground">—</span>
                  </td>
                  <td class="py-1.5">
                    <span
                      :class="
                        socket.listening
                          ? 'text-emerald-600 dark:text-emerald-400'
                          : 'text-muted-foreground'
                      "
                    >
                      {{ socket.state }}
                    </span>
                  </td>
                </tr>
              </tbody>
            </table>

            <p v-if="allSockets.length > 0" class="pt-1 text-[10px] text-muted-foreground">
              点击本地地址即可把该端口加入全局端口过滤，再点一次移除。
            </p>
          </TabsContent>

          <!-- 命令行 -->
          <TabsContent value="cmdline" class="mt-0 space-y-3 p-3">
            <div class="flex items-center justify-between">
              <div class="flex items-center gap-1.5 text-xs font-medium">
                <SquareTerminalIcon class="size-3.5" />
                完整命令行
              </div>
              <Button
                size="xs"
                variant="ghost"
                :disabled="!process?.cmdLine"
                @click="copyText(process?.cmdLine ?? '', '命令行')"
              >
                <CopyIcon />
                复制
              </Button>
            </div>
            <pre
              class="rounded-md border bg-muted/30 p-2.5 font-mono text-[11px] leading-relaxed break-all whitespace-pre-wrap"
              >{{ process?.cmdLine || '（无法读取命令行，通常需要管理员权限）' }}</pre
            >

            <template v-if="process?.cmd && process.cmd.length > 0">
              <Separator />
              <div class="text-xs font-medium">参数拆解（{{ process.cmd.length }}）</div>
              <ol class="space-y-1">
                <li
                  v-for="(arg, index) in process.cmd"
                  :key="index"
                  class="flex gap-2 rounded-md border bg-background/60 px-2 py-1.5 font-mono text-[11px]"
                >
                  <span class="w-6 shrink-0 text-right text-muted-foreground">{{ index }}</span>
                  <span class="break-all">{{ arg }}</span>
                </li>
              </ol>
            </template>
          </TabsContent>

          <!-- 环境变量 -->
          <TabsContent value="env" class="mt-0 space-y-2 p-3">
            <div class="flex items-center gap-2">
              <div class="relative flex-1">
                <SearchIcon
                  class="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground"
                />
                <Input v-model="envFilter" class="h-7 pl-7 text-xs" placeholder="过滤环境变量" />
              </div>
              <Button
                size="xs"
                variant="ghost"
                :disabled="!detail?.environment.length"
                @click="copyAllEnvironment"
              >
                <CopyIcon />
                复制全部
              </Button>
            </div>

            <div
              v-if="!detail?.environment.length"
              class="rounded-md border border-dashed py-10 text-center text-xs text-muted-foreground"
            >
              读取不到环境变量，通常需要以管理员身份运行本工具
            </div>
            <div v-else class="space-y-1">
              <div
                v-for="item in environment"
                :key="item.key"
                class="group flex items-start gap-2 rounded-md border bg-background/60 px-2 py-1.5"
              >
                <span class="w-[180px] shrink-0 truncate font-mono text-[11px] font-medium text-sky-600 dark:text-sky-400">
                  {{ item.key }}
                </span>
                <span class="min-w-0 flex-1 font-mono text-[11px] break-all text-muted-foreground">
                  {{ item.value || '—' }}
                </span>
                <button
                  class="shrink-0 opacity-0 transition-opacity group-hover:opacity-100"
                  type="button"
                  title="复制该变量"
                  @click="copyText(`${item.key}=${item.value}`, item.key)"
                >
                  <CopyIcon class="size-3.5 text-muted-foreground" />
                </button>
              </div>
            </div>
          </TabsContent>

          <!-- 关系 -->
          <TabsContent value="relations" class="mt-0 space-y-3 p-3">
            <div>
              <div class="mb-1.5 flex items-center gap-1.5 text-xs font-medium">
                <LayersIcon class="size-3.5" />
                祖先进程链
              </div>
              <div v-if="ancestorChain.length === 0" class="text-[11px] text-muted-foreground">
                没有可追溯的父进程（已是根进程）
              </div>
              <div v-else class="flex flex-wrap items-center gap-1">
                <template v-for="(node, index) in ancestorChain" :key="node.pid">
                  <Badge
                    variant="outline"
                    class="cursor-pointer font-mono text-[10px] hover:bg-muted"
                    @click="select(node.pid)"
                  >
                    {{ node.name }} · {{ node.pid }}
                  </Badge>
                  <span v-if="index < ancestorChain.length - 1" class="text-muted-foreground">›</span>
                </template>
                <span class="text-muted-foreground">›</span>
                <Badge class="font-mono text-[10px]">
                  {{ process?.displayName }} · {{ process?.pid }}
                </Badge>
              </div>
            </div>

            <Separator />

            <div>
              <div class="mb-1.5 flex items-center gap-1.5 text-xs font-medium">
                <WaypointsIcon class="size-3.5" />
                子进程（{{ detail?.children.length ?? 0 }}）
              </div>
              <div v-if="!detail?.children.length" class="text-[11px] text-muted-foreground">
                该进程没有子进程
              </div>
              <div v-else class="space-y-1">
                <button
                  v-for="child in detail.children"
                  :key="child.pid"
                  type="button"
                  class="flex w-full items-center gap-2 rounded-md border bg-background/60 px-2 py-1.5 text-left hover:bg-muted/50"
                  @click="select(child.pid)"
                >
                  <ServerIcon class="size-3.5 shrink-0 text-muted-foreground" />
                  <span class="min-w-0 flex-1 truncate font-mono text-[11px]">{{ child.name }}</span>
                  <span class="shrink-0 font-mono text-[10px] text-muted-foreground">
                    {{ child.pid }}
                  </span>
                </button>
              </div>
            </div>

            <Separator />

            <div>
              <div class="mb-1.5 flex items-center gap-1.5 text-xs font-medium">
                <InfoIcon class="size-3.5" />
                窗口（{{ process?.windowTitles.length ?? 0 }}）
              </div>
              <div v-if="!process?.windowTitles.length" class="text-[11px] text-muted-foreground">
                该进程没有可见的顶层窗口
              </div>
              <ul v-else class="space-y-1">
                <li
                  v-for="title in process.windowTitles"
                  :key="title"
                  class="truncate rounded-md border bg-background/60 px-2 py-1.5 text-[11px]"
                  :title="title"
                >
                  {{ title }}
                </li>
              </ul>
              <Tooltip>
                <TooltipTrigger as-child>
                  <p class="mt-2 text-[10px] text-muted-foreground">双击列表行可直接切换到该窗口</p>
                </TooltipTrigger>
                <TooltipContent class="text-xs">仅在目标进程存在可见窗口时生效</TooltipContent>
              </Tooltip>
            </div>
          </TabsContent>
        </ScrollArea>
      </Tabs>
    </template>
  </div>
</template>
