<script setup lang="ts">
/**
 * 进程图标。图标由后端从可执行文件中抽取（Windows 走 `SHGetFileInfoW`），
 * 前端按 exe 路径惰性请求并缓存；取不到时退化为统一的占位图标。
 */
import { computed } from 'vue'
import { AppWindowIcon } from '@lucide/vue'

import { iconFor } from '@/composables/useProcessStore'

const props = withDefaults(
  defineProps<{
    exe: string | null
    name?: string
    size?: number
  }>(),
  { name: '', size: 16 },
)

const src = computed(() => iconFor(props.exe))
const style = computed(() => ({ width: `${props.size}px`, height: `${props.size}px` }))
</script>

<template>
  <img
    v-if="src"
    :src="src"
    :alt="name"
    :style="style"
    class="shrink-0 rounded-[3px] object-contain"
    draggable="false"
  />
  <AppWindowIcon v-else :style="style" class="shrink-0 text-muted-foreground/60" />
</template>
