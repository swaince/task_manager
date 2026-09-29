<script setup lang="ts">
/**
 * 顶栏：产品标识、运行环境状态与全局刷新控制。
 */
import { computed } from 'vue'
import {
  CpuIcon,
  PauseIcon,
  PlayIcon,
  RefreshCwIcon,
  ShieldAlertIcon,
  ShieldCheckIcon,
} from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { changeInterval, refreshNow, state, togglePause } from '@/composables/useProcessStore'
import { formatClock } from '@/lib/format'

const intervals = [
  { value: 500, label: '0.5 秒' },
  { value: 1000, label: '1 秒' },
  { value: 1500, label: '1.5 秒' },
  { value: 2000, label: '2 秒' },
  { value: 5000, label: '5 秒' },
  { value: 10000, label: '10 秒' },
  { value: 30000, label: '30 秒' },
]

const intervalValue = computed(() => String(state.intervalMs))
const elevated = computed(() => state.runtime?.elevated ?? false)
const platformLabel = computed(() => {
  switch (state.snapshot?.platform) {
    case 'windows':
      return 'Windows'
    case 'macos':
      return 'macOS'
    case 'linux':
      return 'Linux'
    default:
      return '未知平台'
  }
})

function onIntervalChange(value: unknown): void {
  const millis = Number(value)
  if (Number.isFinite(millis) && millis > 0) void changeInterval(millis)
}
</script>

<template>
  <header class="flex shrink-0 items-center gap-3 border-b bg-card/60 px-3 py-2">
    <div class="flex items-center gap-2">
      <div class="flex size-7 items-center justify-center rounded-md bg-primary text-primary-foreground">
        <CpuIcon class="size-4" />
      </div>
      <div class="leading-tight">
        <h1 class="text-sm font-semibold">进程管理器</h1>
        <p class="text-[10px] text-muted-foreground">
          Process &amp; Port Inspector
          <span v-if="state.runtime?.version"> v{{ state.runtime.version }}</span>
        </p>
      </div>
    </div>

    <Badge variant="outline" class="text-[11px]">{{ platformLabel }}</Badge>

    <Tooltip>
      <TooltipTrigger as-child>
        <Badge
          variant="outline"
          class="text-[11px]"
          :class="
            elevated
              ? 'border-emerald-500/40 text-emerald-600 dark:text-emerald-400'
              : 'border-amber-500/40 text-amber-600 dark:text-amber-400'
          "
        >
          <ShieldCheckIcon v-if="elevated" />
          <ShieldAlertIcon v-else />
          {{ elevated ? '管理员权限' : '普通权限' }}
        </Badge>
      </TooltipTrigger>
      <TooltipContent side="bottom" class="max-w-xs text-xs">
        {{
          elevated
            ? '已以管理员身份运行，可读取受保护进程的命令行与环境变量。'
            : '部分受保护进程的命令行、环境变量与结束操作会失败，建议以管理员身份重新启动。'
        }}
      </TooltipContent>
    </Tooltip>

    <div class="ml-auto flex items-center gap-2">
      <span class="hidden text-[11px] text-muted-foreground md:inline">
        采样 #{{ state.snapshot?.sequence ?? 0 }} ·
        {{ formatClock(state.snapshot?.capturedAt ?? 0) }}
        <span v-if="state.snapshot?.elapsedMs"> · {{ state.snapshot.elapsedMs }}ms</span>
      </span>

      <Select :model-value="intervalValue" @update:model-value="onIntervalChange">
        <SelectTrigger size="sm" class="w-[104px]">
          <SelectValue placeholder="刷新间隔" />
        </SelectTrigger>
        <SelectContent>
          <SelectItem v-for="item in intervals" :key="item.value" :value="String(item.value)">
            {{ item.label }}
          </SelectItem>
        </SelectContent>
      </Select>

      <Button
        size="icon-sm"
        variant="outline"
        :title="state.paused ? '恢复自动刷新' : '暂停自动刷新'"
        @click="togglePause"
      >
        <PlayIcon v-if="state.paused" />
        <PauseIcon v-else />
      </Button>

      <Button size="sm" variant="outline" :disabled="state.refreshing" @click="refreshNow">
        <RefreshCwIcon :class="state.refreshing ? 'animate-spin' : ''" />
        刷新
      </Button>
    </div>
  </header>
</template>
