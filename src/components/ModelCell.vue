<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useDownloadStore } from '@/stores/downloads'
import { isModelAvailable } from '@/stores/types'
import type { ASRModel } from '@/stores/types'
import { Button } from '@/components/ui/button'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Trash2 } from 'lucide-vue-next'
import { Badge } from '@/components/ui/badge'
import BenchmarkBadges from '@/components/BenchmarkBadges.vue'
import DownloadActions from '@/components/DownloadActions.vue'
import { formatSize } from '@/utils/format'

const { t } = useI18n()
const downloads = useDownloadStore()

const props = defineProps<{
  model: ASRModel
}>()

const emit = defineEmits<{
  download: [model: ASRModel]
  delete: [model: ASRModel]
}>()

const isDownloading = computed(() => !!downloads.activeDownloads[props.model.id])
const isDownloaded = computed(() => isModelAvailable(props.model))
const isDeleting = computed(() => !!downloads.deletingModels[props.model.id])
const isPaused = computed(() => {
  return !isDownloading.value && !isDownloaded.value && props.model.partial_progress != null && props.model.partial_progress > 0
})

// ModelCell-specific states (deleting, hover-to-trash) override the base DownloadActions
const showCustomDownloaded = computed(() => isDownloaded.value && !isDownloading.value && !isPaused.value)
</script>

<template>
  <div
    class="group flex items-center gap-3 px-3.5 py-3 rounded-[10px] transition-shadow bg-panel-card-bg border-[0.5px] border-panel-card-border backdrop-blur hover:shadow-panel-card"
  >
    <!-- Model info -->
    <div class="flex-1 min-w-0">
      <div class="flex items-center gap-2">
        <span class="font-medium text-sm truncate">{{ model.label }}</span>
        <span v-if="model.size > 0" class="text-xs text-muted-foreground shrink-0">{{ formatSize(model.size) }}</span>
      </div>
      <BenchmarkBadges v-if="model.wer != null || model.rtf != null || model.params != null || model.quantization || model.ram != null || (model.lang_codes && model.lang_codes.length > 0)" :wer="model.wer" :rtf="model.rtf" :params="model.params" :quantization="model.quantization" :ram="model.ram" :lang-codes="model.lang_codes" class="mt-0.5" />
    </div>

    <!-- Status / Actions -->
    <div class="relative flex-shrink-0">
      <!-- Deleting — greyed trash with indeterminate bar, centered over badge -->
      <template v-if="isDeleting">
        <Badge
          variant="secondary"
          class="bg-green-500/10 text-green-500 border-transparent invisible h-8 px-3 text-xs"
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
      <template v-else-if="showCustomDownloaded">
        <Badge
          variant="secondary"
          class="bg-green-500/10 text-green-500 border-transparent group-hover:invisible h-8 px-3 text-xs"
        >
          {{ t('modelManager.downloaded') }}
        </Badge>
        <div class="absolute inset-0 flex items-center justify-center invisible group-hover:visible">
          <TooltipProvider :delay-duration="300">
            <Tooltip>
              <TooltipTrigger as-child>
                <Button
                  variant="ghost" size="icon-sm"
                  class="cursor-pointer"
                  @click="emit('delete', model)"
                >
                  <Trash2 class="w-4 h-4" />
                </Button>
              </TooltipTrigger>
              <TooltipContent side="bottom" :side-offset="4">{{ t('modelManager.delete') }}</TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
      </template>

      <!-- Common states: downloading, paused, not-downloaded -->
      <DownloadActions
        v-else
        :model="model"
        @download="emit('download', $event)"
      />
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
