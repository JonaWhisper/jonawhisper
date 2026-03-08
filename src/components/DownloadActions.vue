<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useDownloadStore } from '@/stores/downloads'
import { isModelAvailable } from '@/stores/types'
import type { ASRModel } from '@/stores/types'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
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
      <TooltipProvider v-else :delay-duration="300">
        <Tooltip>
          <TooltipTrigger as-child>
            <Button variant="ghost" size="icon-sm" :aria-label="t('aria.pause')" @click="downloads.pauseDownload(model.id)">
              <Pause class="w-3.5 h-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom" :side-offset="4">{{ t('modelManager.pause') }}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger as-child>
            <Button variant="ghost" size="icon-sm" :aria-label="t('aria.cancel')" @click="downloads.cancelDownload(model.id)">
              <X class="w-3.5 h-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom" :side-offset="4">{{ t('modelManager.cancel') }}</TooltipContent>
        </Tooltip>
      </TooltipProvider>
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
      <TooltipProvider :delay-duration="300">
        <Tooltip>
          <TooltipTrigger as-child>
            <Button variant="ghost" size="icon-sm" :aria-label="t('aria.resume')" @click="emit('download', model)">
              <Play class="w-3.5 h-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom" :side-offset="4">{{ t('modelManager.resume') }}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger as-child>
            <Button variant="ghost" size="icon-sm" :aria-label="t('aria.cancel')" @click="downloads.cancelDownload(model.id)">
              <X class="w-3.5 h-3.5" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom" :side-offset="4">{{ t('modelManager.cancel') }}</TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </div>
  </template>

  <!-- Downloaded -->
  <template v-else-if="isDownloaded">
    <Badge variant="secondary" class="bg-green-500/10 text-green-500 border-transparent h-8 px-3 text-xs">
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
