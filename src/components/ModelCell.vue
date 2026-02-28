<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore, type ASRModel } from '@/stores/app'
import { Button } from '@/components/ui/button'
import { Trash2, Pause, Play, X, Loader2 } from 'lucide-vue-next'
import { Badge } from '@/components/ui/badge'
import { Progress } from '@/components/ui/progress'
import BenchmarkBadges from '@/components/BenchmarkBadges.vue'

const { t } = useI18n()
const store = useAppStore()

const props = defineProps<{
  model: ASRModel
}>()

const emit = defineEmits<{
  download: [model: ASRModel]
  delete: [model: ASRModel]
}>()

const dl = computed(() => store.activeDownloads[props.model.id])
const isDownloading = computed(() => !!dl.value)
const progress = computed(() => dl.value?.progress ?? 0)
const isStopping = computed(() => dl.value?.stopping ?? false)

const isDownloaded = computed(() => {
  const dt = props.model.download_type.type
  if (dt === 'System') return true
  return props.model.is_downloaded
})

const isPaused = computed(() => {
  return !isDownloading.value && !isDownloaded.value && props.model.partial_progress != null && props.model.partial_progress > 0
})

function formatSize(bytes: number): string {
  if (bytes <= 0) return ''
  if (bytes >= 1_000_000_000) return t('size.gb', [+(bytes / 1_000_000_000).toFixed(1)])
  return t('size.mb', [Math.round(bytes / 1_000_000)])
}
</script>

<template>
  <div
    class="group flex items-center gap-3 px-4 py-3 rounded-lg border transition-colors hover:bg-accent/30 bg-card border-border"
  >
    <!-- Model info -->
    <div class="flex-1 min-w-0">
      <div class="font-medium text-sm truncate">{{ model.label }}</div>
      <div class="flex items-center gap-1.5 flex-wrap text-xs text-muted-foreground">
        <span v-if="model.size > 0">{{ formatSize(model.size) }}</span>
        <template v-if="model.wer != null || model.rtf != null">
          <span v-if="model.size > 0" class="opacity-40">&middot;</span>
          <BenchmarkBadges :wer="model.wer" :rtf="model.rtf" />
        </template>
      </div>
    </div>

    <!-- Status / Actions -->
    <div class="relative flex-shrink-0">
      <!-- Downloading -->
      <template v-if="isDownloading">
        <div class="flex items-center gap-2">
          <Progress :model-value="progress * 100" class="w-24" />
          <span class="text-xs text-muted-foreground w-10 text-right">
            {{ Math.round(progress * 100) }}%
          </span>
          <template v-if="isStopping">
            <Loader2 class="w-3.5 h-3.5 animate-spin text-muted-foreground" />
          </template>
          <template v-else>
            <Button variant="ghost" size="icon-sm" @click="store.pauseDownload(model.id)" :title="t('modelManager.pause')">
              <Pause class="w-3.5 h-3.5" />
            </Button>
            <Button variant="ghost" size="icon-sm" @click="store.cancelDownload(model.id)" :title="t('modelManager.cancel')">
              <X class="w-3.5 h-3.5" />
            </Button>
          </template>
        </div>
      </template>

      <!-- Paused (partial exists) -->
      <template v-else-if="isPaused">
        <div class="flex items-center gap-2">
          <Progress :model-value="(model.partial_progress ?? 0) * 100" class="w-24" />
          <span class="text-xs text-muted-foreground w-10 text-right">
            {{ Math.round((model.partial_progress ?? 0) * 100) }}%
          </span>
          <Button variant="ghost" size="icon-sm" @click="emit('download', model)" :title="t('modelManager.resume')">
            <Play class="w-3.5 h-3.5" />
          </Button>
          <Button variant="ghost" size="icon-sm" @click="store.cancelDownload(model.id)" :title="t('modelManager.cancel')">
            <X class="w-3.5 h-3.5" />
          </Button>
        </div>
      </template>

      <!-- Downloaded — badge swaps to trash on hover -->
      <template v-else-if="isDownloaded">
        <Badge
          variant="secondary"
          class="bg-green-500/10 text-green-500 border-transparent group-hover:opacity-0 transition-opacity"
        >
          {{ t('modelManager.downloaded') }}
        </Badge>
        <Button
          variant="ghost" size="icon-sm"
          class="absolute inset-0 m-auto opacity-0 group-hover:opacity-100 transition-opacity"
          @click="emit('delete', model)"
          :title="t('modelManager.delete')"
        >
          <Trash2 class="w-4 h-4" />
        </Button>
      </template>

      <!-- Not downloaded -->
      <template v-else>
        <Button size="sm" @click="emit('download', model)">
          {{ t('modelManager.download') }}
        </Button>
      </template>
    </div>
  </div>
</template>
