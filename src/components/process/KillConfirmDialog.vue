<script setup lang="ts">
/**
 * 结束进程的二次确认弹窗。
 *
 * 结束进程属于不可逆操作，这里强制确认，并把「仅此进程 / 整棵进程树」
 * 以及「是否强制」两个选项显式摆出来，避免误伤。
 */
import { computed, ref, watch } from 'vue'
import { SkullIcon, Trash2Icon, TriangleAlertIcon } from '@lucide/vue'

import { Button } from '@/components/ui/button'
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'
import { Switch } from '@/components/ui/switch'

const props = defineProps<{
  open: boolean
  pid: number | null
  /** 由调用方预选的模式：从行的「结束进程树」入口进来时为 true。 */
  tree: boolean
}>()

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void
  (e: 'confirm', mode: 'process' | 'tree', force: boolean): void
}>()

const mode = ref<'process' | 'tree'>('process')
const force = ref(true)

watch(
  () => props.open,
  (open) => {
    if (!open) return
    mode.value = props.tree ? 'tree' : 'process'
    force.value = true
  },
)

const openModel = computed({
  get: () => props.open,
  set: (value: boolean) => emit('update:open', value),
})
</script>

<template>
  <Dialog v-model:open="openModel">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle class="flex items-center gap-2">
          <TriangleAlertIcon class="size-4 text-destructive" />
          确认结束进程
        </DialogTitle>
        <DialogDescription>
          进程 <span class="font-mono font-medium text-foreground">PID {{ pid }}</span>
          将被立即终止，未保存的数据会丢失且无法恢复。
        </DialogDescription>
      </DialogHeader>

      <div class="space-y-3">
        <div class="grid grid-cols-2 gap-2">
          <button
            type="button"
            class="flex flex-col gap-1 rounded-lg border p-3 text-left transition-colors"
            :class="mode === 'process' ? 'border-primary bg-primary/5' : 'hover:bg-muted/50'"
            @click="mode = 'process'"
          >
            <span class="flex items-center gap-1.5 text-sm font-medium">
              <SkullIcon class="size-3.5" />
              仅此进程
            </span>
            <span class="text-[11px] text-muted-foreground">只结束 PID {{ pid }}</span>
          </button>
          <button
            type="button"
            class="flex flex-col gap-1 rounded-lg border p-3 text-left transition-colors"
            :class="mode === 'tree' ? 'border-destructive bg-destructive/5' : 'hover:bg-muted/50'"
            @click="mode = 'tree'"
          >
            <span class="flex items-center gap-1.5 text-sm font-medium">
              <Trash2Icon class="size-3.5" />
              整棵进程树
            </span>
            <span class="text-[11px] text-muted-foreground">
              连同全部子进程一起结束
            </span>
          </button>
        </div>

        <label class="flex items-center justify-between rounded-lg border px-3 py-2.5">
          <span class="text-xs">
            <span class="font-medium">强制结束</span>
            <span class="block text-[11px] text-muted-foreground">
              跳过优雅退出流程，直接终止（Windows 上两者等价）
            </span>
          </span>
          <Switch
            :model-value="force"
            @update:model-value="(value: unknown) => (force = Boolean(value))"
          />
        </label>
      </div>

      <DialogFooter class="gap-2 sm:justify-end">
        <Button variant="outline" size="sm" @click="openModel = false">取消</Button>
        <Button
          variant="destructive"
          size="sm"
          @click="
            () => {
              emit('confirm', mode, force)
              openModel = false
            }
          "
        >
          {{ mode === 'tree' ? '结束整棵进程树' : '结束进程' }}
        </Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
