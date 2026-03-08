<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useEnginesStore } from '@/stores/engines'
import { parseCloudId } from '@/stores/types'
import type { HistoryEntry } from '@/stores/types'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Copy, Check, Trash2 } from 'lucide-vue-next'
import TypeBadge from '@/components/TypeBadge.vue'

const { t } = useI18n()
const enginesStore = useEnginesStore()

const props = defineProps<{
  entry: HistoryEntry
  copiedTimestamp: number | null
}>()

const emit = defineEmits<{
  copy: [entry: HistoryEntry]
  delete: [entry: HistoryEntry]
}>()

function formatTime(timestamp: number): string {
  const date = new Date(timestamp * 1000)
  return date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
}

function formatAsrLabel(modelId: string): string {
  const cloudId = parseCloudId(modelId)
  if (cloudId) {
    const provider = enginesStore.providers.find(p => p.id === cloudId)
    return provider ? provider.name : 'Cloud'
  }
  const model = enginesStore.models.find(m => m.id === modelId)
  return model ? model.label : modelId
}

function isCloudAsr(modelId: string): boolean {
  return !!parseCloudId(modelId)
}

function formatCleanupLabel(id: string): string {
  if (id.startsWith('bert-punctuation:')) return 'BERT'
  const cloudId = parseCloudId(id)
  if (cloudId) {
    const provider = enginesStore.providers.find(p => p.id === cloudId)
    return provider ? provider.name : 'Cloud LLM'
  }
  const model = enginesStore.models.find(m => m.id === id)
  return model ? model.label : id
}

function formatModelLabel(id: string): string {
  const model = enginesStore.models.find(m => m.id === id)
  return model ? model.label : id.split(':').pop() || id
}

function cleanupBadgeType(id: string): 'bert' | 'punctuation' | 'correction' | 'llm' | 'cloud' {
  if (id.startsWith('bert-punctuation:')) return 'bert'
  if (id.startsWith('pcs-punctuation:')) return 'punctuation'
  if (id.startsWith('correction:')) return 'correction'
  if (parseCloudId(id)) return 'cloud'
  return 'llm'
}
</script>

<template>
  <div
    class="flex items-start gap-2.5 p-[10px_12px] bg-panel-card-bg border-[0.5px] border-panel-card-border rounded-[10px] mb-1.5 transition-shadow duration-150 hover:shadow-panel-card group"
  >
    <span class="text-[11px] text-muted-foreground mt-0.5 shrink-0 tabular-nums min-w-[38px]">
      {{ formatTime(entry.timestamp) }}
    </span>
    <div class="flex-1 min-w-0">
      <p class="text-[13px] leading-snug line-clamp-2 mb-1">{{ entry.text }}</p>
      <TooltipProvider v-if="entry.model_id" :delay-duration="300">
        <div class="flex flex-wrap gap-1">
          <Tooltip>
            <TooltipTrigger as-child>
              <TypeBadge :type="isCloudAsr(entry.model_id) ? 'cloud' : 'local'">
                {{ formatAsrLabel(entry.model_id) }}
              </TypeBadge>
            </TooltipTrigger>
            <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.asr') }}</TooltipContent>
          </Tooltip>
          <Tooltip v-if="entry.language">
            <TooltipTrigger as-child>
              <span class="inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium bg-zinc-500/10 text-zinc-600 dark:text-zinc-400">
                {{ entry.language }}
              </span>
            </TooltipTrigger>
            <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.language') }}</TooltipContent>
          </Tooltip>
          <Tooltip v-if="entry.vad_trimmed">
            <TooltipTrigger as-child>
              <TypeBadge type="vad">VAD</TypeBadge>
            </TooltipTrigger>
            <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.vad') }}</TooltipContent>
          </Tooltip>
          <Tooltip v-if="entry.cleanup_model_id">
            <TooltipTrigger as-child>
              <TypeBadge :type="cleanupBadgeType(entry.cleanup_model_id)">
                {{ formatCleanupLabel(entry.cleanup_model_id) }}
              </TypeBadge>
            </TooltipTrigger>
            <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.cleanup') }}</TooltipContent>
          </Tooltip>
          <Tooltip v-if="entry.punctuation_model_id">
            <TooltipTrigger as-child>
              <TypeBadge type="punctuation">
                {{ formatModelLabel(entry.punctuation_model_id) }}
              </TypeBadge>
            </TooltipTrigger>
            <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.punctuation') }}</TooltipContent>
          </Tooltip>
          <Tooltip v-if="entry.spellcheck">
            <TooltipTrigger as-child>
              <TypeBadge type="spellcheck">{{ t('history.badge.spellcheck') }}</TypeBadge>
            </TooltipTrigger>
            <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.spellcheckTooltip') }}</TooltipContent>
          </Tooltip>
          <Tooltip v-if="entry.disfluency_removal">
            <TooltipTrigger as-child>
              <TypeBadge type="disfluency">{{ t('history.badge.disfluency') }}</TypeBadge>
            </TooltipTrigger>
            <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.disfluencyTooltip') }}</TooltipContent>
          </Tooltip>
          <Tooltip v-if="entry.hallucination_filter">
            <TooltipTrigger as-child>
              <TypeBadge type="hallucination" />
            </TooltipTrigger>
            <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.hallucination') }}</TooltipContent>
          </Tooltip>
          <Tooltip v-if="entry.itn">
            <TooltipTrigger as-child>
              <TypeBadge type="itn">ITN</TypeBadge>
            </TooltipTrigger>
            <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.itnTooltip') }}</TooltipContent>
          </Tooltip>
        </div>
      </TooltipProvider>
    </div>
    <div class="flex gap-1 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity pt-0.5">
      <TooltipProvider :delay-duration="300">
        <Tooltip>
          <TooltipTrigger as-child>
            <button :aria-label="t('aria.copy')" class="relative w-6 h-6 flex items-center justify-center rounded text-muted-foreground hover:text-foreground hover:bg-muted/50" @click="emit('copy', entry)">
              <Check v-if="copiedTimestamp === entry.timestamp" class="h-3.5 w-3.5 text-green-600" />
              <Copy v-else class="h-3.5 w-3.5" />
              <Transition name="copied-toast">
                <span
                  v-if="copiedTimestamp === entry.timestamp"
                  class="absolute -top-6 left-1/2 -translate-x-1/2 px-1.5 py-0.5 rounded text-[10px] font-medium bg-green-600 text-white whitespace-nowrap pointer-events-none"
                >{{ t('history.copy') }}</span>
              </Transition>
            </button>
          </TooltipTrigger>
          <TooltipContent side="bottom" :side-offset="4">{{ t('history.copy') }}</TooltipContent>
        </Tooltip>
        <Tooltip>
          <TooltipTrigger as-child>
            <button :aria-label="t('aria.delete')" class="w-6 h-6 flex items-center justify-center rounded text-muted-foreground hover:text-destructive hover:bg-muted/50" @click="emit('delete', entry)">
              <Trash2 class="h-3.5 w-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="bottom" :side-offset="4">{{ t('history.delete') }}</TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </div>
  </div>
</template>

<style scoped>
.copied-toast-enter-active { transition: all 0.15s ease-out; }
.copied-toast-leave-active { transition: all 0.25s ease-in; }
.copied-toast-enter-from,
.copied-toast-leave-to { opacity: 0; transform: translate(-50%, 2px); }
</style>
