<script setup lang="ts">
/**
 * 端口徽标组：监听端口用强调色，其他绑定端口用弱化色，超出的折叠为 `+N`。
 *
 * 徽标可点击 —— 点击即把该端口加入 / 移出端口过滤条件，
 * 这是「从进程找到端口，再按端口反查所有进程」的最短路径。
 */
import { computed } from 'vue'

import { Badge } from '@/components/ui/badge'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import { isPortFiltered, togglePortFilter } from '@/composables/useProcessStore'

const props = withDefaults(
  defineProps<{
    ports: number[]
    listeningPorts: number[]
    limit?: number
    interactive?: boolean
  }>(),
  { limit: 3, interactive: true },
)

interface Chip {
  port: number
  listening: boolean
  filtered: boolean
}

const chips = computed<Chip[]>(() => {
  const listening = new Set(props.listeningPorts)
  return props.ports.map((port) => ({
    port,
    listening: listening.has(port),
    filtered: isPortFiltered(port),
  }))
})

const shown = computed(() => chips.value.slice(0, props.limit))
const hidden = computed(() => chips.value.length - shown.value.length)
const tooltipText = computed(() => chips.value.map((c) => c.port).join(', '))

function chipTitle(chip: Chip): string {
  return chip.filtered
    ? `端口 ${chip.port} 已在过滤条件中，点击移除`
    : `按端口 ${chip.port} 过滤`
}

function onChipClick(chip: Chip): void {
  if (!props.interactive) return
  togglePortFilter(chip.port)
}
</script>

<template>
  <div v-if="chips.length > 0" class="flex items-center gap-1">
    <Badge
      v-for="chip in shown"
      :key="chip.port"
      :variant="chip.filtered ? 'outline' : chip.listening ? 'default' : 'outline'"
      :class="[
        'h-5 rounded-md px-1.5 font-mono text-[11px] tabular-nums',
        interactive ? 'cursor-pointer' : 'cursor-default',
        chip.filtered
          ? 'border-sky-500/60 bg-sky-500/15 text-sky-700 dark:text-sky-300'
          : chip.listening
            ? 'border-transparent bg-emerald-500/15 text-emerald-600 dark:text-emerald-400'
            : 'border-border bg-muted/40 text-muted-foreground',
      ]"
      :title="chipTitle(chip)"
      @click.stop="onChipClick(chip)"
    >
      <span
        v-if="chip.listening"
        class="mr-0.5 size-1.5 rounded-full bg-emerald-500"
        aria-hidden="true"
      />
      {{ chip.port }}
    </Badge>
    <Tooltip v-if="hidden > 0">
      <TooltipTrigger as-child>
        <Badge variant="ghost" class="h-5 rounded-md px-1.5 text-[11px] text-muted-foreground">
          +{{ hidden }}
        </Badge>
      </TooltipTrigger>
      <TooltipContent side="top" class="font-mono text-xs">
        {{ tooltipText }}
      </TooltipContent>
    </Tooltip>
  </div>
  <span v-else class="text-xs text-muted-foreground/50">—</span>
</template>
