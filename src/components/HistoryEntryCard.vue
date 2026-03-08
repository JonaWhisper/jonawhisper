<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEnginesStore } from '@/stores/engines'
import { parseCloudId } from '@/stores/types'
import type { HistoryEntry } from '@/stores/types'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import {
  Copy, Check, Trash2, Mic, Scissors, ShieldCheck, Eraser, Type, BookA, SpellCheck, MessageSquare, Cloud, Hash, ChevronRight, X, Slash,
} from 'lucide-vue-next'
import { diffWords } from 'diff'

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
    if (props.entry.raw_text && props.entry.raw_text !== props.entry.text) {
      return [
        { step: 'asr', text: props.entry.raw_text },
        { step: 'final', text: props.entry.text },
      ]
    }
    return []
  }
})

// Substantive steps: ASR + steps that actually changed text
const substantiveStepNames = new Set(['preprocess', 'punctuation', 'spellcheck', 'correction', 'itn'])

const substantiveSteps = computed(() => {
  const all = pipelineSteps.value
  if (all.length < 2) return []
  const filtered = [all[0]!]
  for (let i = 1; i < all.length; i++) {
    if (substantiveStepNames.has(all[i]!.step) && all[i]!.text !== filtered[filtered.length - 1]!.text) {
      filtered.push(all[i]!)
    }
  }
  return filtered.length >= 2 ? filtered : []
})

// Set of step names that have actual text diffs available
const stepsWithDiff = computed(() => {
  const names = new Set<string>()
  for (const s of substantiveSteps.value) {
    if (s.step !== 'asr') names.add(s.step)
  }
  return names
})

// Steps that ran but produced no change (step:nochange markers)
const stepsNoChange = computed(() => {
  const names = new Set<string>()
  for (const s of pipelineSteps.value) {
    if (s.step.endsWith(':nochange')) {
      names.add(s.step.replace(':nochange', ''))
    }
  }
  return names
})

// Steps that failed with an error (step:error markers)
const stepsError = computed(() => {
  const names = new Set<string>()
  for (const s of pipelineSteps.value) {
    if (s.step.endsWith(':error')) {
      names.add(s.step.replace(':error', ''))
    }
  }
  return names
})

// Protected words from spellcheck guards
interface ProtectedWord {
  word: string
  reason: string
}

const protectedWords = computed<ProtectedWord[]>(() => {
  for (const s of pipelineSteps.value) {
    if (s.step === 'spellcheck:protected' && s.text) {
      try {
        return JSON.parse(s.text) as ProtectedWord[]
      } catch { return [] }
    }
  }
  return []
})

function formatProtectedReason(reason: string): string {
  if (reason === 'user-dict') return t('history.badge.protectedUserDict')
  if (reason.startsWith('cross-lang:')) return t('history.badge.protectedCrossLang')
  if (reason.startsWith('confidence:')) {
    const score = reason.split(':')[1]
    return t('history.badge.protectedConfidence', { score })
  }
  return reason
}

// -- Pipeline stepper UI --

interface PipelineIcon {
  id: string
  icon: typeof Mic
  active: boolean
  hasDiff: boolean
  noChange: boolean   // ran but produced no change
  hasError: boolean   // ran but failed
  tooltip: string
  color: string       // active color classes
}

const pipelineIcons = computed<PipelineIcon[]>(() => {
  const e = props.entry
  const icons: PipelineIcon[] = []

  // 1. ASR — always present
  const asrLabel = e.model_id ? formatAsrLabel(e.model_id) : 'ASR'
  const isCloud = e.model_id ? !!parseCloudId(e.model_id) : false
  icons.push({
    id: 'asr',
    icon: isCloud ? Cloud : Mic,
    active: true,
    hasDiff: false,
    noChange: false,
    hasError: false,
    tooltip: asrLabel + (e.language ? ` (${e.language})` : ''),
    color: isCloud ? 'text-sky-500' : 'text-blue-500',
  })

  // 2. VAD
  icons.push({
    id: 'vad',
    icon: Scissors,
    active: !!e.vad_trimmed,
    hasDiff: false,
    noChange: false,
    hasError: false,
    tooltip: t('history.badge.vad'),
    color: 'text-emerald-500',
  })

  // 3. Hallucination filter
  icons.push({
    id: 'hallucination',
    icon: ShieldCheck,
    active: !!e.hallucination_filter,
    hasDiff: false,
    noChange: false,
    hasError: false,
    tooltip: t('history.badge.hallucination'),
    color: 'text-rose-500',
  })

  // 4. Disfluency removal
  icons.push({
    id: 'disfluency',
    icon: Eraser,
    active: !!e.disfluency_removal,
    hasDiff: false,
    noChange: false,
    hasError: false,
    tooltip: t('history.badge.disfluencyTooltip'),
    color: 'text-pink-500',
  })

  // 5. Punctuation
  const punctLabel = e.punctuation_model_id ? formatModelLabel(e.punctuation_model_id) : t('history.badge.punctuation')
  icons.push({
    id: 'punctuation',
    icon: Type,
    active: !!e.punctuation_model_id,
    hasDiff: stepsWithDiff.value.has('punctuation'),
    noChange: stepsNoChange.value.has('punctuation'),
    hasError: stepsError.value.has('punctuation'),
    tooltip: punctLabel + (stepsNoChange.value.has('punctuation') ? ` (${t('history.badge.noChange')})` : '') + (stepsError.value.has('punctuation') ? ` (${t('history.badge.error')})` : ''),
    color: 'text-violet-500',
  })

  // 6. Spellcheck
  const pw = protectedWords.value
  let spellTooltip = t('history.badge.spellcheckTooltip')
  if (stepsNoChange.value.has('spellcheck')) spellTooltip += ` (${t('history.badge.noChange')})`
  if (stepsError.value.has('spellcheck')) spellTooltip += ` (${t('history.badge.error')})`
  if (pw.length > 0) {
    spellTooltip += `\n${t('history.badge.protected', { count: pw.length })}: `
      + pw.map(p => `${p.word} (${formatProtectedReason(p.reason)})`).join(', ')
  }
  icons.push({
    id: 'spellcheck',
    icon: BookA,
    active: !!e.spellcheck,
    hasDiff: stepsWithDiff.value.has('spellcheck'),
    noChange: stepsNoChange.value.has('spellcheck'),
    hasError: stepsError.value.has('spellcheck'),
    tooltip: spellTooltip,
    color: 'text-lime-600',
  })

  // 7. Correction / LLM cleanup
  const cleanupLabel = e.cleanup_model_id ? formatCleanupLabel(e.cleanup_model_id) : t('history.badge.cleanup')
  icons.push({
    id: 'correction',
    icon: e.cleanup_model_id && parseCloudId(e.cleanup_model_id) ? MessageSquare : SpellCheck,
    active: !!e.cleanup_model_id,
    hasDiff: stepsWithDiff.value.has('correction'),
    noChange: stepsNoChange.value.has('correction'),
    hasError: stepsError.value.has('correction'),
    tooltip: cleanupLabel + (stepsNoChange.value.has('correction') ? ` (${t('history.badge.noChange')})` : '') + (stepsError.value.has('correction') ? ` (${t('history.badge.error')})` : ''),
    color: 'text-amber-500',
  })

  // 8. ITN
  icons.push({
    id: 'itn',
    icon: Hash,
    active: !!e.itn,
    hasDiff: stepsWithDiff.value.has('itn'),
    noChange: stepsNoChange.value.has('itn'),
    hasError: false,
    tooltip: t('history.badge.itnTooltip') + (stepsNoChange.value.has('itn') ? ` (${t('history.badge.noChange')})` : ''),
    color: 'text-cyan-500',
  })

  return icons
})

// Active diff state
const activeDiffStep = ref<string | null>(null)

// Map step id to its index in substantiveSteps for diff computation
const diffForStep = computed(() => {
  if (!activeDiffStep.value) return []
  const steps = substantiveSteps.value
  // Find the index of the active step in substantiveSteps
  const idx = steps.findIndex(s => s.step === activeDiffStep.value)
  if (idx < 1) return [] // need a previous step to diff against
  return diffWords(steps[idx - 1]!.text, steps[idx]!.text)
})

function toggleDiffStep(stepId: string) {
  if (activeDiffStep.value === stepId) {
    activeDiffStep.value = null
  } else {
    activeDiffStep.value = stepId
  }
}

// Reset diff when entry changes
watch(() => props.entry.timestamp, () => {
  activeDiffStep.value = null
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
      <!-- Pipeline stepper -->
      <TooltipProvider v-if="entry.model_id" :delay-duration="200">
        <div class="flex items-center gap-0 mb-1.5">
          <template v-for="(step, i) in pipelineIcons" :key="step.id">
            <ChevronRight v-if="i > 0" class="h-2.5 w-2.5 text-muted-foreground/30 shrink-0 -mx-0.5" />
            <Tooltip>
              <TooltipTrigger as-child>
                <button
                  class="relative w-5 h-5 flex items-center justify-center rounded-full transition-all duration-150 shrink-0"
                  :class="[
                    !step.active
                      ? 'text-muted-foreground/25 cursor-default'
                      : step.hasError
                        ? 'text-muted-foreground/30 cursor-default'
                        : step.hasDiff
                          ? 'cursor-pointer hover:bg-muted/80 ' + step.color
                          : step.noChange
                            ? 'text-muted-foreground/40 cursor-default'
                            : 'text-foreground/70 cursor-default',
                    activeDiffStep === step.id ? 'ring-1.5 ring-current bg-current/10 scale-110' : '',
                  ]"
                  :disabled="!step.hasDiff"
                  @click="step.hasDiff && toggleDiffStep(step.id)"
                >
                  <component :is="step.icon" class="h-3 w-3" :class="step.hasError ? 'opacity-60' : ''" />
                  <!-- Error: small red X overlaid on the icon -->
                  <X v-if="step.hasError" class="absolute h-2.5 w-2.5 text-destructive/50 stroke-[2]" />
                  <!-- No change: diagonal slash through icon -->
                  <Slash v-if="step.noChange" class="absolute h-3.5 w-3.5 text-muted-foreground/50 stroke-[1.5]" />
                </button>
              </TooltipTrigger>
              <TooltipContent side="bottom" :side-offset="4">
                <span class="text-[11px] whitespace-pre-line">{{ step.tooltip }}</span>
              </TooltipContent>
            </Tooltip>
          </template>
        </div>
      </TooltipProvider>

      <!-- Diff view for selected pipeline step -->
      <div v-if="activeDiffStep && diffForStep.length > 0" class="mb-1">
        <span class="text-[10px] text-muted-foreground mb-0.5 block">{{ pipelineIcons.find(s => s.id === activeDiffStep)?.tooltip }}</span>
        <p class="text-[13px] leading-snug">
          <span
            v-for="(part, i) in diffForStep"
            :key="i"
            :class="{
              'bg-green-500/20 text-green-700 dark:text-green-300 rounded-sm': part.added,
              'bg-red-500/20 text-red-700 dark:text-red-300 line-through rounded-sm': part.removed,
            }"
          >{{ part.value }}</span>
        </p>
      </div>
      <!-- Main text: always show the final corrected version -->
      <p v-else class="text-[13px] leading-snug mb-1">{{ entry.text }}</p>
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
