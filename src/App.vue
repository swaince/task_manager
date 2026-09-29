<script setup lang="ts">
/**
 * 应用外壳：顶栏 + 指标条 + 工具栏 + 主从分栏 + 状态栏。
 *
 * 主区域用 shadcn-vue 的 `ResizablePanelGroup` 做左右分栏，
 * 左侧是可切换的列表 / 进程树，右侧是详情面板。
 */
import { onBeforeUnmount, onMounted, ref } from 'vue'

import AppHeader from '@/components/layout/AppHeader.vue'
import AppStatusBar from '@/components/layout/AppStatusBar.vue'
import KillConfirmDialog from '@/components/process/KillConfirmDialog.vue'
import PortsTable from '@/components/process/PortsTable.vue'
import ProcessDetailPanel from '@/components/process/ProcessDetailPanel.vue'
import ProcessTable from '@/components/process/ProcessTable.vue'
import ProcessToolbar from '@/components/process/ProcessToolbar.vue'
import StatsStrip from '@/components/process/StatsStrip.vue'
import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@/components/ui/resizable'
import { Toaster } from '@/components/ui/sonner'
import { TooltipProvider } from '@/components/ui/tooltip'
import {
  bootstrap,
  dispose,
  killProcess,
  killProcessTree,
  state,
} from '@/composables/useProcessStore'

const killOpen = ref(false)
const killPid = ref<number | null>(null)
const killTree = ref(false)

function requestKill(pid: number, tree: boolean): void {
  killPid.value = pid
  killTree.value = tree
  killOpen.value = true
}

async function confirmKill(mode: 'process' | 'tree', force: boolean): Promise<void> {
  const pid = killPid.value
  if (pid == null) return
  if (mode === 'tree') await killProcessTree(pid, force)
  else await killProcess(pid, force)
}

onMounted(() => {
  void bootstrap()
})

onBeforeUnmount(() => {
  dispose()
})
</script>

<template>
  <TooltipProvider :delay-duration="300">
    <div class="flex h-screen w-screen flex-col overflow-hidden bg-background text-foreground">
      <AppHeader />
      <StatsStrip />
      <ProcessToolbar />

      <ResizablePanelGroup direction="horizontal" class="min-h-0 flex-1">
        <ResizablePanel :default-size="62" :min-size="34" class="min-h-0">
          <PortsTable v-if="state.view === 'ports'" @request-kill="requestKill" />
          <ProcessTable v-else @request-kill="requestKill" />
        </ResizablePanel>

        <ResizableHandle with-handle />

        <ResizablePanel :default-size="38" :min-size="22" :max-size="70" class="min-h-0">
          <ProcessDetailPanel @request-kill="requestKill" />
        </ResizablePanel>
      </ResizablePanelGroup>

      <AppStatusBar />
    </div>

    <KillConfirmDialog
      v-model:open="killOpen"
      :pid="killPid"
      :tree="killTree"
      @confirm="confirmKill"
    />

    <Toaster theme="dark" position="bottom-right" :close-button="true" rich-colors />
  </TooltipProvider>
</template>
