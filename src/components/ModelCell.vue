<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore, type ASRModel } from '@/stores/app'
import { Button } from '@/components/ui/button'
import { Trash2, Pause, Play, X, Loader2 } from 'lucide-vue-next'
import { Badge } from '@/components/ui/badge'
import { Progress } from '@/components/ui/progress'
import BenchmarkBadges from '@/components/BenchmarkBadges.vue'
import { formatSize, formatSpeed } from '@/utils/format'

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

const isDeleting = computed(() => !!store.deletingModels[props.model.id])

const isPaused = computed(() => {
  return !isDownloading.value && !isDownloaded.value && props.model.partial_progress != null && props.model.partial_progress > 0
})

const speedText = computed(() => dl.value ? formatSpeed(dl.value.speed) : '')
</script>

<template>
  <div
    class="group flex items-center gap-3 px-4 py-3 rounded-lg border transition-colors hover:bg-accent/30 bg-card border-border"
  >
    <!-- Model info -->
    <div class="flex-1 min-w-0">
      <div class="flex items-center gap-2">
        <span class="font-medium text-sm truncate">{{ model.label }}</span>
        <span v-if="model.size > 0" class="text-xs text-muted-foreground shrink-0">{{ formatSize(model.size) }}</span>
      </div>
      <BenchmarkBadges v-if="model.wer != null || model.rtf != null || model.params != null || model.ram != null || (model.lang_codes && model.lang_codes.length > 0)" :wer="model.wer" :rtf="model.rtf" :params="model.params" :ram="model.ram" :lang-codes="model.lang_codes" class="mt-0.5" />
    </div>

    <!-- Status / Actions -->
    <div class="relative flex-shrink-0">
      <!-- Downloading -->
      <template v-if="isDownloading">
        <div class="flex items-center gap-2">
          <div class="w-24">
            <Progress :model-value="progress * 100" />
            <div class="text-[10px] text-muted-foreground mt-0.5">
              {{ speedText }}
            </div>
          </div>
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
          <div class="w-24">
            <Progress :model-value="(model.partial_progress ?? 0) * 100" />
            <div v-if="model.size > 0" class="text-[10px] text-muted-foreground mt-0.5">
              {{ formatSize(Math.round((model.partial_progress ?? 0) * model.size)) }} / {{ formatSize(model.size) }}
            </div>
          </div>
          <Button variant="ghost" size="icon-sm" @click="emit('download', model)" :title="t('modelManager.resume')">
            <Play class="w-3.5 h-3.5" />
          </Button>
          <Button variant="ghost" size="icon-sm" @click="store.cancelDownload(model.id)" :title="t('modelManager.cancel')">
            <X class="w-3.5 h-3.5" />
          </Button>
        </div>
      </template>

      <!-- Deleting — greyed trash with indeterminate bar, centered over badge -->
      <template v-else-if="isDeleting">
        <Badge
          variant="secondary"
          class="bg-green-500/10 text-green-500 border-transparent invisible"
        >
          {{ t('modelManager.downloaded') }}
        </Badge>
        <div class="absolute inset-0 m-auto flex items-center justify-center w-8 h-8 rounded-md">
          <Trash2 class="w-4 h-4 text-muted-foreground/40" />
          <div class="absolute bottom-0.5 left-1 right-1 h-0.5 rounded-full overflow-hidden bg-muted-foreground/15">
            <div class="h-full w-1/3 rounded-full bg-muted-foreground/40 animate-indeterminate" />
          </div>
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

<style scoped>
.animate-indeterminate {
  animation: indeterminate 1.5s ease-in-out infinite;
}
@keyframes indeterminate {
  0% { transform: translateX(-100%); }
  100% { transform: translateX(400%); }
}
</style>
