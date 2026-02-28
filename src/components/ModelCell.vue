<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { ASRModel } from '@/stores/app'
import { Button } from '@/components/ui/button'
import { Trash2 } from 'lucide-vue-next'
import { Badge } from '@/components/ui/badge'
import { Progress } from '@/components/ui/progress'
import BenchmarkBadges from '@/components/BenchmarkBadges.vue'

const { t } = useI18n()

const props = defineProps<{
  model: ASRModel
  isSelected: boolean
  isDownloading: boolean
  downloadProgress: number
}>()

const emit = defineEmits<{
  select: [model: ASRModel]
  download: [model: ASRModel]
  delete: [model: ASRModel]
}>()

const isDownloaded = computed(() => {
  const dt = props.model.download_type.type
  if (dt === 'RemoteAPI' || dt === 'System') return true
  return props.model.is_downloaded
})

const isRemoteAPI = computed(() => props.model.download_type.type === 'RemoteAPI')

function formatSize(bytes: number): string {
  if (bytes <= 0) return ''
  if (bytes >= 1_000_000_000) return t('size.gb', [+(bytes / 1_000_000_000).toFixed(1)])
  return t('size.mb', [Math.round(bytes / 1_000_000)])
}
</script>

<template>
  <div
    class="flex items-center gap-3 px-4 py-3 rounded-lg border border-border bg-card transition-colors hover:bg-accent/30"
    :class="{ 'ring-2 ring-primary/30': isSelected }"
  >
    <!-- Radio button -->
    <button
      @click="isDownloaded ? emit('select', model) : null"
      :disabled="!isDownloaded"
      class="flex-shrink-0"
    >
      <div
        class="w-4 h-4 rounded-full border-2 flex items-center justify-center transition-colors"
        :class="isSelected
          ? 'border-primary bg-primary'
          : isDownloaded
            ? 'border-muted-foreground hover:border-primary'
            : 'border-muted opacity-50'"
      >
        <div v-if="isSelected" class="w-2 h-2 rounded-full bg-primary-foreground" />
      </div>
    </button>

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
    <div class="flex items-center gap-2 flex-shrink-0">
      <!-- Downloading -->
      <template v-if="isDownloading">
        <Progress :model-value="downloadProgress * 100" class="w-24" />
        <span class="text-xs text-muted-foreground w-10 text-right">
          {{ Math.round(downloadProgress * 100) }}%
        </span>
      </template>

      <!-- Downloaded -->
      <template v-else-if="isDownloaded && !isRemoteAPI">
        <Badge variant="secondary" class="bg-green-500/10 text-green-500 border-transparent">
          {{ t('modelManager.downloaded') }}
        </Badge>
        <Button variant="ghost" size="icon-sm" @click="emit('delete', model)" :title="t('modelManager.delete')">
          <Trash2 class="w-4 h-4" />
        </Button>
      </template>

      <!-- Remote API -->
      <template v-else-if="isRemoteAPI">
        <Badge variant="secondary" class="bg-blue-500/10 text-blue-500 border-transparent">API</Badge>
        <Button variant="ghost" size="icon-sm" @click="emit('delete', model)" :title="t('modelManager.delete')">
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
