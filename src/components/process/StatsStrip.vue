<script setup lang="ts">
/**
 * 顶部指标条：把整机视角的关键数字压成一行紧凑卡片。
 */
import { computed } from 'vue'
import {
  ActivityIcon,
  HardDriveIcon,
  LayersIcon,
  NetworkIcon,
  ServerIcon,
  ClockIcon,
} from '@lucide/vue'

import { Progress } from '@/components/ui/progress'
import { state, filteredProcesses } from '@/composables/useProcessStore'
import { formatBytes, formatDuration, formatNumber, formatPercent } from '@/lib/format'

const totals = computed(() => state.snapshot?.totals ?? null)

const memoryRatio = computed(() => {
  const t = totals.value
  if (!t || t.totalMemory === 0) return 0
  return Math.min(100, (t.usedMemory / t.totalMemory) * 100)
})

const shownCount = computed(() => filteredProcesses.value.length)
const filtered = computed(() => shownCount.value !== (totals.value?.processCount ?? 0))
</script>

<template>
  <div class="grid shrink-0 grid-cols-2 gap-2 border-b bg-card/40 px-3 py-2 md:grid-cols-3 xl:grid-cols-6">
    <div class="rounded-lg border bg-background/60 px-3 py-2">
      <div class="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground">
        <LayersIcon class="size-3.5" />
        进程
      </div>
      <div class="mt-0.5 flex items-baseline gap-1.5">
        <span class="font-mono text-lg leading-none font-semibold tabular-nums">
          {{ formatNumber(shownCount) }}
        </span>
        <span v-if="filtered" class="text-[11px] text-muted-foreground">
          / {{ formatNumber(totals?.processCount ?? 0) }}
        </span>
      </div>
    </div>

    <div class="rounded-lg border bg-background/60 px-3 py-2">
      <div class="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground">
        <ActivityIcon class="size-3.5" />
        CPU 总占用
      </div>
      <div class="mt-0.5 flex items-baseline gap-1.5">
        <span class="font-mono text-lg leading-none font-semibold tabular-nums">
          {{ formatPercent(totals?.globalCpuUsage ?? 0) }}
        </span>
        <span class="text-[11px] text-muted-foreground">{{ totals?.cpuCount ?? 0 }} 核</span>
      </div>
    </div>

    <div class="rounded-lg border bg-background/60 px-3 py-2">
      <div class="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground">
        <HardDriveIcon class="size-3.5" />
        物理内存
      </div>
      <div class="mt-0.5 flex items-baseline gap-1.5">
        <span class="font-mono text-lg leading-none font-semibold tabular-nums">
          {{ formatBytes(totals?.usedMemory ?? 0) }}
        </span>
        <span class="text-[11px] text-muted-foreground">
          / {{ formatBytes(totals?.totalMemory ?? 0) }}
        </span>
      </div>
      <Progress :model-value="memoryRatio" class="mt-1.5 h-1" />
    </div>

    <div class="rounded-lg border bg-background/60 px-3 py-2">
      <div class="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground">
        <ServerIcon class="size-3.5" />
        监听端口
      </div>
      <div class="mt-0.5 flex items-baseline gap-1.5">
        <span class="font-mono text-lg leading-none font-semibold tabular-nums text-emerald-600 dark:text-emerald-400">
          {{ formatNumber(totals?.listeningPortCount ?? 0) }}
        </span>
      </div>
    </div>

    <div class="rounded-lg border bg-background/60 px-3 py-2">
      <div class="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground">
        <NetworkIcon class="size-3.5" />
        TCP 连接
      </div>
      <div class="mt-0.5 flex items-baseline gap-1.5">
        <span class="font-mono text-lg leading-none font-semibold tabular-nums">
          {{ formatNumber(totals?.connectionCount ?? 0) }}
        </span>
      </div>
    </div>

    <div class="rounded-lg border bg-background/60 px-3 py-2">
      <div class="flex items-center gap-1.5 text-[11px] font-medium text-muted-foreground">
        <ClockIcon class="size-3.5" />
        {{ totals?.hostName ?? '本机' }}
      </div>
      <div class="mt-0.5 truncate text-xs font-medium" :title="`${totals?.osName} ${totals?.osVersion}`">
        {{ totals?.osName }} {{ totals?.osVersion }}
      </div>
      <div class="text-[11px] text-muted-foreground">
        已运行 {{ formatDuration(totals?.uptime ?? 0) }}
      </div>
    </div>
  </div>
</template>
