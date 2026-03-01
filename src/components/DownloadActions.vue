<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useDownloadStore } from '@/stores/downloads'
import { isModelAvailable } from '@/stores/types'
import type { ASRModel } from '@/stores/types'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Progress } from '@/components/ui/progress'
import { Pause, Play, X, Loader2 } from 'lucide-vue-next'
import { formatSize, formatSpeed } from '@/utils/format'

const { t } = useI18n()
const downloads = useDownloadStore()

const props = defineProps<{
  model: ASRModel
  compact?: boolean
}>()

const emit = defineEmits<{
  download: [model: ASRModel]
}>()

const dl = computed(() => downloads.activeDownloads[props.model.id])
const isDownloading = computed(() => !!dl.value)
const progress = computed(() => dl.value?.progress ?? 0)
const isStopping = computed(() => dl.value?.stopping ?? false)
const isDownloaded = computed(() => isModelAvailable(props.model))

const isPaused = computed(() => {
  return !isDownloading.value && !isDownloaded.value && props.model.partial_progress != null && props.model.partial_progress > 0
})

const speedText = computed(() => dl.value ? formatSpeed(dl.value.speed) : '')

const barWidth = computed(() => props.compact ? 'w-16' : 'w-24')
const textSize = computed(() => props.compact ? 'text-[9px]' : 'text-[10px]')
</script>

<template>
  <!-- Downloading -->
  <template v-if="isDownloading">
    <div class="flex items-center gap-2">
      <div :class="barWidth">
        <Progress :model-value="progress * 100" />
        <div :class="[textSize, 'text-muted-foreground mt-0.5']">
          {{ speedText }}
        </div>
      </div>
      <template v-if="isStopping">
        <Loader2 class="w-3.5 h-3.5 animate-spin text-muted-foreground" />
      </template>
      <template v-else>
        <Button variant="ghost" size="icon-sm" @click="downloads.pauseDownload(model.id)" :title="t('modelManager.pause')">
          <Pause class="w-3.5 h-3.5" />
        </Button>
        <Button variant="ghost" size="icon-sm" @click="downloads.cancelDownload(model.id)" :title="t('modelManager.cancel')">
          <X class="w-3.5 h-3.5" />
        </Button>
      </template>
    </div>
  </template>

  <!-- Paused (partial exists) -->
  <template v-else-if="isPaused">
    <div class="flex items-center gap-2">
      <div :class="barWidth">
        <Progress :model-value="(model.partial_progress ?? 0) * 100" />
        <div v-if="model.size > 0" :class="[textSize, 'text-muted-foreground mt-0.5']">
          {{ formatSize(Math.round((model.partial_progress ?? 0) * model.size)) }} / {{ formatSize(model.size) }}
        </div>
      </div>
      <Button variant="ghost" size="icon-sm" @click="emit('download', model)" :title="t('modelManager.resume')">
        <Play class="w-3.5 h-3.5" />
      </Button>
      <Button variant="ghost" size="icon-sm" @click="downloads.cancelDownload(model.id)" :title="t('modelManager.cancel')">
        <X class="w-3.5 h-3.5" />
      </Button>
    </div>
  </template>

  <!-- Downloaded -->
  <template v-else-if="isDownloaded">
    <Badge variant="secondary" class="bg-green-500/10 text-green-500 border-transparent text-xs">
      {{ t('modelManager.downloaded') }}
    </Badge>
  </template>

  <!-- Not downloaded -->
  <template v-else>
    <Button size="sm" @click="emit('download', model)">
      {{ t('modelManager.download') }}
    </Button>
  </template>
</template>
