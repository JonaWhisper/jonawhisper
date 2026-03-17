<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { Activity, Cpu } from 'lucide-vue-next'
import { useSettingsStore } from '@/stores/settings'
import { Switch } from '@/components/ui/switch'

const { t } = useI18n()
const settings = useSettingsStore()

interface MemoryInfo {
  rss_mb: number | null
  contexts: [string, string][]
}

const memoryInfo = ref<MemoryInfo | null>(null)
let pollInterval: ReturnType<typeof setInterval> | null = null

async function fetchMemoryInfo() {
  try {
    memoryInfo.value = await invoke<MemoryInfo>('get_memory_info')
  } catch (e) {
    console.error('get_memory_info failed:', e)
  }
}

onMounted(() => {
  fetchMemoryInfo()
  pollInterval = setInterval(fetchMemoryInfo, 3000)
})

onUnmounted(() => {
  if (pollInterval) clearInterval(pollInterval)
})

function formatRss(mb: number | null): string {
  if (mb == null || mb <= 0) return t('diagnostic.na')
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`
  return `${Math.round(mb)} MB`
}

function onAutoReleaseChange(checked: boolean) {
  settings.setSetting('auto_release_memory', String(checked))
}
</script>

<template>
  <div class="space-y-6">
    <h2 class="text-lg font-semibold text-foreground">{{ t('panel.diagnostic') }}</h2>

    <!-- Memory -->
    <div class="rounded-xl border border-panel-card-border bg-panel-card p-4 space-y-3">
      <div class="flex items-center gap-2 text-sm font-medium text-foreground">
        <Activity class="w-4 h-4 text-panel-accent" />
        {{ t('diagnostic.memory') }}
      </div>

      <div v-if="memoryInfo" class="space-y-2">
        <div class="flex items-center justify-between">
          <span class="text-[13px] text-muted-foreground">{{ t('diagnostic.rss') }}</span>
          <span class="text-[13px] font-mono tabular-nums" :class="memoryInfo.rss_mb != null && memoryInfo.rss_mb > 3000 ? 'text-red-500' : memoryInfo.rss_mb != null && memoryInfo.rss_mb > 2000 ? 'text-amber-500' : 'text-foreground'">
            {{ formatRss(memoryInfo.rss_mb) }}
          </span>
        </div>

        <!-- Auto release toggle -->
        <div class="flex items-center justify-between">
          <div>
            <div class="text-[13px] text-foreground">{{ t('diagnostic.autoRelease') }}</div>
            <div class="text-[11px] text-muted-foreground">{{ t('diagnostic.autoReleaseHint') }}</div>
          </div>
          <Switch
            :checked="settings.autoReleaseMemory"
            @update:checked="onAutoReleaseChange"
          />
        </div>

        <!-- Loaded contexts -->
        <div class="pt-2 border-t border-panel-divider">
          <div class="flex items-center gap-2 text-[13px] text-muted-foreground mb-2">
            <Cpu class="w-3.5 h-3.5" />
            {{ t('diagnostic.loadedContexts') }}
          </div>
          <div v-if="memoryInfo.contexts.length === 0" class="text-[12px] text-muted-foreground/60 italic">
            {{ t('diagnostic.noContexts') }}
          </div>
          <div v-else class="space-y-1">
            <div
              v-for="[engineId, contextKey] in memoryInfo.contexts"
              :key="engineId"
              class="flex items-center justify-between text-[12px]"
            >
              <span class="font-mono text-foreground">{{ engineId }}</span>
              <span class="text-muted-foreground truncate ml-2 max-w-[200px]">{{ contextKey }}</span>
            </div>
          </div>
        </div>
      </div>

      <div v-else class="text-[13px] text-muted-foreground/60 italic">
        {{ t('diagnostic.loading') }}
      </div>
    </div>
  </div>
</template>
