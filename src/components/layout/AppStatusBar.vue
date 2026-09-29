<script setup lang="ts">
/**
 * 底部状态栏：采样健康度、告警与快捷键提示。
 */
import { computed } from 'vue'
import { CircleAlertIcon, InfoIcon, TriangleAlertIcon, WaypointsIcon } from '@lucide/vue'

import { Badge } from '@/components/ui/badge'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { activePortRules, filterActive, state } from '@/composables/useProcessStore'
import { ruleToToken } from '@/lib/port-filter'
import { formatClock } from '@/lib/format'

const warnings = computed(() => state.snapshot?.warnings ?? [])
const hasProblem = computed(() => Boolean(state.error) || warnings.value.length > 0)
const hasData = computed(() => (state.snapshot?.processes.length ?? 0) > 0)

/** 生效中的端口过滤表达式（规范化后），让用户随时看清「为什么只剩这些进程」。 */
const portFilterText = computed(() => activePortRules.value.map(ruleToToken).join(', '))

/** 未写 local:/remote: 的规则会落在这个默认范围上。 */
const scopeLabel = computed(() => {
  switch (state.portScope) {
    case 'local':
      return '仅本地端口'
    case 'remote':
      return '仅远端端口'
    default:
      return '本地/远端均可'
  }
})
</script>

<template>
  <footer
    class="flex shrink-0 items-center gap-2 border-t bg-card/60 px-3 py-1.5 text-[11px] text-muted-foreground"
  >
    <span class="flex items-center gap-1">
      <span
        class="size-1.5 rounded-full"
        :class="
          state.error
            ? 'bg-destructive'
            : state.paused
              ? 'bg-amber-500'
              : hasProblem
                ? 'bg-amber-500'
                : 'bg-emerald-500'
        "
      />
      {{
        state.error
          ? '采样失败'
          : state.paused
            ? '已暂停自动刷新'
            : `每 ${(state.intervalMs / 1000).toFixed(1)} 秒自动刷新`
      }}
    </span>

    <span class="text-muted-foreground/50">|</span>
    <span>
      最近采样 {{ formatClock(state.snapshot?.capturedAt ?? 0) }}
      <template v-if="state.snapshot?.elapsedMs"> · 耗时 {{ state.snapshot.elapsedMs }} ms</template>
    </span>

    <span class="text-muted-foreground/50">|</span>
    <span class="font-mono">#{{ state.snapshot?.sequence ?? 0 }}</span>

    <template v-if="filterActive">
      <span class="text-muted-foreground/50">|</span>
      <Badge variant="outline" class="h-4 text-[10px]">过滤生效中</Badge>
    </template>

    <template v-if="portFilterText">
      <span class="text-muted-foreground/50">|</span>
      <span class="flex items-center gap-1 font-mono text-sky-600 dark:text-sky-400">
        <WaypointsIcon class="size-3" />
        端口过滤 {{ portFilterText }}
        <span class="text-muted-foreground">· 未加前缀的规则：{{ scopeLabel }}</span>
      </span>
    </template>

    <div v-if="state.error" class="ml-2 flex items-center gap-1 text-destructive">
      <CircleAlertIcon class="size-3.5" />
      <span class="font-mono">{{ state.error }}</span>
    </div>

    <Tooltip v-else-if="warnings.length > 0">
      <TooltipTrigger as-child>
        <span class="flex items-center gap-1 text-amber-600 dark:text-amber-400">
          <TriangleAlertIcon class="size-3.5" />
          {{ warnings.length }} 条告警
        </span>
      </TooltipTrigger>
      <TooltipContent side="top" class="max-w-md text-xs">
        <p v-for="(warning, index) in warnings" :key="index">{{ warning }}</p>
      </TooltipContent>
    </Tooltip>

    <div class="ml-auto flex items-center gap-1.5">
      <InfoIcon class="size-3" />
      <span v-if="hasData">
        ↑↓ 选择 · Enter 定位窗口 · Delete 结束进程 · 双击行定位窗口
      </span>
      <span v-else>等待首次采样…</span>
    </div>
  </footer>
</template>
