<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEnginesStore } from '@/stores/engines'
import { parseCloudId } from '@/stores/types'
import type { HistoryEntry } from '@/stores/types'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Copy, Check, Trash2, GitCompareArrows } from 'lucide-vue-next'
import { diffWords } from 'diff'
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

// -- Word confidence scores (hover only, no colors) --

interface WordScore {
  word: string
  score: number // -1 = unknown
}

const wordScores = computed<WordScore[]>(() => {
  if (!props.entry.word_scores) return []
  try {
    const parsed = JSON.parse(props.entry.word_scores) as [string, number][]
    return parsed.map(([word, score]) => ({ word, score }))
  } catch { return [] }
})

function confidenceLabel(score: number): string {
  if (score < 0) return t('history.confidence.unknown')
  return `${(score * 100).toFixed(0)}%`
}

function confidenceColor(score: number): string {
  if (score < 0) return '' // unknown → default color
  if (score >= 0.9) return '#22c55e' // green
  if (score >= 0.7) return '#eab308' // yellow
  return '#ef4444' // red
}


// -- Pipeline steps diff --

interface PipelineStep {
  step: string
  text: string
}

const pipelineSteps = computed<PipelineStep[]>(() => {
  if (!props.entry.raw_text) return []
  try {
    const parsed = JSON.parse(props.entry.raw_text) as [string, string][]
    if (!Array.isArray(parsed) || parsed.length < 2) return []
    return parsed.map(([step, text]) => ({ step, text }))
  } catch {
    // Legacy format: raw_text is a plain string
    if (props.entry.raw_text && props.entry.raw_text !== props.entry.text) {
      return [
        { step: 'asr', text: props.entry.raw_text },
        { step: 'final', text: props.entry.text },
      ]
    }
    return []
  }
})

// Filter out cosmetic steps (finalize, itn) — only show substantive changes
const substantiveStepNames = new Set(['preprocess', 'punctuation', 'spellcheck', 'correction'])

const substantiveSteps = computed(() => {
  const all = pipelineSteps.value
  if (all.length < 2) return []
  // Keep ASR (first) + substantive steps that actually changed the text
  const filtered = [all[0]!]
  for (let i = 1; i < all.length; i++) {
    if (substantiveStepNames.has(all[i]!.step) && all[i]!.text !== filtered[filtered.length - 1]!.text) {
      filtered.push(all[i]!)
    }
  }
  return filtered.length >= 2 ? filtered : []
})

const hasSteps = computed(() => substantiveSteps.value.length >= 2)
const showDiff = ref(false)
const selectedStep = ref(0)

const stepLabels: Record<string, string> = {
  asr: 'ASR',
  preprocess: 'Preprocess',
  punctuation: 'Punctuation',
  spellcheck: 'Spellcheck',
  correction: 'Correction',
  final: 'Final',
}

const currentDiff = computed(() => {
  const steps = substantiveSteps.value
  if (steps.length < 2) return []
  const idx = Math.min(selectedStep.value, steps.length - 2)
  return diffWords(steps[idx]!.text, steps[idx + 1]!.text)
})

</script>

<template>
  <div
    class="flex items-start gap-2.5 p-[10px_12px] bg-panel-card-bg border-[0.5px] border-panel-card-border rounded-[10px] mb-1.5 transition-shadow duration-150 hover:shadow-panel-card group"
  >
    <span class="text-[11px] text-muted-foreground mt-0.5 shrink-0 tabular-nums min-w-[38px]">
      {{ formatTime(entry.timestamp) }}
    </span>
    <div class="flex-1 min-w-0">
      <!-- Diff view: inline diff per step -->
      <div v-if="showDiff && hasSteps" class="mb-1">
        <div class="flex items-center gap-1.5 mb-1.5">
          <button
            v-for="idx in substantiveSteps.length - 1"
            :key="idx"
            class="px-1.5 py-0.5 rounded text-[10px] font-medium transition-colors"
            :class="selectedStep === idx - 1
              ? 'bg-blue-500/20 text-blue-700 dark:text-blue-300'
              : 'bg-muted/50 text-muted-foreground hover:bg-muted'"
            @click="selectedStep = idx - 1"
          >{{ stepLabels[substantiveSteps[idx]!.step] ?? substantiveSteps[idx]!.step }}</button>
        </div>
        <p class="text-[13px] leading-snug">
          <span
            v-for="(part, i) in currentDiff"
            :key="i"
            :class="{
              'bg-green-500/20 text-green-700 dark:text-green-300 rounded-sm': part.added,
              'bg-red-500/20 text-red-700 dark:text-red-300 line-through rounded-sm': part.removed,
            }"
          >{{ part.value }}</span>
        </p>
      </div>
      <!-- Normal view with confidence hover (colored by score) -->
      <TooltipProvider v-else-if="wordScores.length > 0" :delay-duration="200">
        <p class="text-[13px] leading-snug line-clamp-2 mb-1">
          <template v-for="(ws, i) in wordScores" :key="i">
            <Tooltip>
              <TooltipTrigger as-child>
                <span
                  class="cursor-default rounded-sm transition-colors duration-150 hover:bg-muted"
                  :class="ws.score >= 0 && ws.score < 0.7 ? 'border-b border-dotted border-amber-500/60' : ''"
                >{{ ws.word }}</span>
              </TooltipTrigger>
              <TooltipContent side="bottom" :side-offset="2" :style="ws.score >= 0 ? { backgroundColor: confidenceColor(ws.score), color: 'white' } : {}">
                <span class="text-[11px] font-medium">{{ confidenceLabel(ws.score) }}</span>
              </TooltipContent>
            </Tooltip>{{ i < wordScores.length - 1 ? ' ' : '' }}
          </template>
        </p>
      </TooltipProvider>
      <!-- Fallback: plain text -->
      <p v-else class="text-[13px] leading-snug line-clamp-2 mb-1">{{ entry.text }}</p>
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
        <Tooltip v-if="hasSteps">
          <TooltipTrigger as-child>
            <button
              :aria-label="t('history.diff')"
              class="w-6 h-6 flex items-center justify-center rounded hover:bg-muted/50 transition-colors"
              :class="showDiff ? 'text-blue-600 dark:text-blue-400' : 'text-muted-foreground hover:text-foreground'"
              @click="showDiff = !showDiff"
            >
              <GitCompareArrows class="h-3.5 w-3.5" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="bottom" :side-offset="4">{{ t('history.diff') }}</TooltipContent>
        </Tooltip>
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
