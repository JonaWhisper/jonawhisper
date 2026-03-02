<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore } from '@/stores/settings'
import { useEnginesStore } from '@/stores/engines'
import { getLlmModels } from '@/config/providers'
import type { CleanupModel } from '@/stores/types'
import { Switch } from '@/components/ui/switch'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Slider } from '@/components/ui/slider'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { formatRam } from '@/utils/format'
import { RefreshCw, Loader2 } from 'lucide-vue-next'

const { t } = useI18n()
const settings = useSettingsStore()
const engines = useEnginesStore()

const DISABLED_VALUE = '_disabled'

// Unified cleanup value: "_disabled" or the cleanup model id
const unifiedCleanupValue = computed(() => {
  if (!settings.textCleanupEnabled) return DISABLED_VALUE
  return settings.cleanupModelId || DISABLED_VALUE
})

async function onUnifiedCleanupChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  if (value === DISABLED_VALUE) {
    await settings.setSetting('text_cleanup_enabled', 'false')
  } else {
    await settings.setSetting('text_cleanup_enabled', 'true')
    await settings.setSetting('cleanup_model_id', value)
  }
}

const selectedCleanupModel = computed(() =>
  engines.cleanupModels.find(m => m.id === settings.cleanupModelId) ?? null
)

const cleanupGroupLabel = (group: CleanupModel['group']) => t(`settings.cleanupGroup.${group}`)
const cleanupGroupClass = (group: CleanupModel['group']) => {
  switch (group) {
    case 'bert': return 'bg-violet-500/10 text-violet-600'
    case 'correction': return 'bg-amber-500/10 text-amber-600'
    case 'llm': return 'bg-blue-500/10 text-blue-600'
    case 'cloud': return 'bg-sky-500/10 text-sky-600'
  }
}

function formatParams(params: number): string {
  return params % 1 === 0 ? params.toFixed(0) + 'B' : params.toFixed(1) + 'B'
}

function formatLangs(codes: string[]): string {
  if (codes.length <= 6) return codes.map(c => c.toUpperCase()).join(' ')
  return `${codes.length} ${t('settings.langs')}`
}

// LLM config (shown when cloud cleanup selected)
const llmSelectedProvider = computed(() =>
  engines.providers.find(p => p.id === engines.cleanupCloudProviderId)
)

const llmModelOptions = computed(() => {
  const provider = llmSelectedProvider.value
  return provider ? getLlmModels(provider) : []
})

const isCustomLlmModel = computed(() => llmModelOptions.value.length === 0)

const refreshingLlm = ref(false)

async function refreshLlmModels() {
  const provider = llmSelectedProvider.value
  if (!provider || refreshingLlm.value) return
  refreshingLlm.value = true
  try {
    const models = await invoke<string[]>('fetch_provider_models', { provider })
    await engines.updateProvider({ ...provider, cached_models: models })
  } catch (e) {
    console.error('refreshLlmModels failed:', e)
  } finally {
    refreshingLlm.value = false
  }
}

let llmModelDebounce: ReturnType<typeof setTimeout> | null = null

async function onLlmModelSelect(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('llm_model', value)
}

function onLlmModelInput(event: Event) {
  const value = (event.target as HTMLInputElement).value
  settings.llmModel = value
  if (llmModelDebounce) clearTimeout(llmModelDebounce)
  llmModelDebounce = setTimeout(() => {
    settings.setSetting('llm_model', value)
  }, 500)
}

function onMaxTokensSliderUpdate(v: number[] | undefined) {
  if (v?.[0] != null) settings.llmMaxTokens = v[0]
}
function onMaxTokensSliderCommit(v: number[]) {
  const val = v[0] ?? settings.llmMaxTokens
  settings.setSetting('llm_max_tokens', String(val))
}
</script>

<template>
  <div>
    <!-- Pre-processing card -->
    <div class="wf-card">
      <div class="wf-card-title">{{ t('settings.postProcessing.vad') }}</div>
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('settings.postProcessing.vad') }}</div>
        </div>
        <Switch
          :model-value="settings.vadEnabled"
          @update:model-value="(v: boolean) => settings.setSetting('vad_enabled', String(v))"
        />
      </div>
    </div>

    <!-- Post-processing card -->
    <div class="wf-card">
      <div class="wf-card-title">{{ t('settings.postProcessing.textCleanup') }}</div>

      <!-- Hallucination filter -->
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('settings.postProcessing.hallucinations') }}</div>
        </div>
        <Switch
          :model-value="settings.hallucinationFilterEnabled"
          @update:model-value="(v: boolean) => settings.setSetting('hallucination_filter_enabled', String(v))"
        />
      </div>

      <!-- Unified cleanup dropdown -->
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('settings.postProcessing.textCleanup') }}</div>
        </div>
        <Select
          :model-value="unifiedCleanupValue"
          @update:model-value="onUnifiedCleanupChange"
        >
          <SelectTrigger class="w-auto min-w-[190px] h-8 text-xs">
            <span v-if="unifiedCleanupValue === DISABLED_VALUE" class="text-muted-foreground">
              {{ t('settings.shortcut.cancel.none') }}
            </span>
            <span v-else-if="selectedCleanupModel" class="inline-flex items-center gap-1.5 truncate">
              <span class="truncate">{{ selectedCleanupModel.label }}</span>
              <Badge variant="secondary" :class="['text-[9px] px-1 py-0 border-transparent font-medium shrink-0', cleanupGroupClass(selectedCleanupModel.group)]">
                {{ cleanupGroupLabel(selectedCleanupModel.group) }}
              </Badge>
            </span>
          </SelectTrigger>
          <SelectContent>
            <SelectItem :value="DISABLED_VALUE">{{ t('settings.shortcut.cancel.none') }}</SelectItem>
            <SelectItem v-for="m in engines.cleanupModels" :key="m.id" :value="m.id">
              <div class="flex flex-col gap-0.5">
                <span class="flex items-center gap-1.5">
                  {{ m.label }}
                  <Badge v-if="m.recommended" variant="secondary" class="text-[9px] px-1 py-0 bg-emerald-500/10 text-emerald-600 border-transparent font-medium">{{ t('settings.cleanup.recommended') }}</Badge>
                  <Badge variant="secondary" :class="['text-[9px] px-1 py-0 border-transparent font-medium', cleanupGroupClass(m.group)]">{{ cleanupGroupLabel(m.group) }}</Badge>
                </span>
                <span v-if="m.params != null || m.ram != null || (m.lang_codes && m.lang_codes.length > 0)" class="inline-flex items-center gap-1 flex-wrap">
                  <Badge v-if="m.params != null" variant="secondary" class="text-[9px] px-1 py-0 bg-slate-500/10 text-slate-600 border-transparent font-medium">{{ formatParams(m.params) }}</Badge>
                  <Badge v-if="m.ram != null" variant="secondary" class="text-[9px] px-1 py-0 bg-cyan-500/10 text-cyan-600 border-transparent font-medium">RAM <span class="opacity-50 font-normal">~{{ formatRam(m.ram) }}</span></Badge>
                  <Badge v-if="m.lang_codes && m.lang_codes.length > 0" variant="secondary" class="text-[9px] px-1 py-0 bg-indigo-500/10 text-indigo-600 border-transparent font-medium">{{ formatLangs(m.lang_codes) }}</Badge>
                </span>
              </div>
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>

    <!-- Cloud LLM sub-settings -->
    <template v-if="settings.textCleanupEnabled && engines.isCloudLlm && llmSelectedProvider">
      <div class="wf-card">
        <div class="wf-card-title">{{ t('settings.llm.model') }}</div>
        <div class="wf-form-row">
          <div>
            <div class="wf-form-label">{{ t('settings.llm.model') }}</div>
          </div>
          <div class="flex items-center gap-2">
            <Select
              v-if="!isCustomLlmModel"
              :model-value="settings.llmModel"
              @update:model-value="onLlmModelSelect"
            >
              <SelectTrigger class="w-auto min-w-[140px] h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="m in llmModelOptions" :key="m" :value="m">{{ m }}</SelectItem>
              </SelectContent>
            </Select>
            <Input
              v-else
              :value="settings.llmModel"
              @input="onLlmModelInput"
              class="h-8 text-xs min-w-[140px]"
            />
            <TooltipProvider :delay-duration="300">
              <Tooltip>
                <TooltipTrigger as-child>
                  <Button variant="outline" size="icon" class="h-8 w-8 shrink-0" :disabled="refreshingLlm" @click="refreshLlmModels">
                    <Loader2 v-if="refreshingLlm" class="w-3.5 h-3.5 animate-spin" />
                    <RefreshCw v-else class="w-3.5 h-3.5" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom" :side-offset="4">{{ t('settings.models.refresh') }}</TooltipContent>
              </Tooltip>
            </TooltipProvider>
          </div>
        </div>
        <div class="wf-form-row">
          <div>
            <div class="wf-form-label">{{ t('settings.llm.maxTokens') }}</div>
          </div>
          <div class="flex items-center gap-2">
            <Slider
              class="w-24"
              :model-value="[settings.llmMaxTokens]"
              :min="128"
              :max="8192"
              :step="128"
              @update:model-value="onMaxTokensSliderUpdate"
              @value-commit="onMaxTokensSliderCommit"
            />
            <span class="text-xs text-muted-foreground tabular-nums min-w-8 text-right">{{ settings.llmMaxTokens }}</span>
          </div>
        </div>
      </div>
    </template>

    <!-- Local LLM sub-settings (token cap only) -->
    <template v-if="settings.textCleanupEnabled && engines.isLocalLlm">
      <div class="wf-card">
        <div class="wf-card-title">{{ t('settings.llm.maxTokens') }}</div>
        <div class="wf-form-row">
          <div>
            <div class="wf-form-label">{{ t('settings.llm.maxTokens') }}</div>
          </div>
          <div class="flex items-center gap-2">
            <Slider
              class="w-24"
              :model-value="[settings.llmMaxTokens]"
              :min="128"
              :max="8192"
              :step="128"
              @update:model-value="onMaxTokensSliderUpdate"
              @value-commit="onMaxTokensSliderCommit"
            />
            <span class="text-xs text-muted-foreground tabular-nums min-w-8 text-right">{{ settings.llmMaxTokens }}</span>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>
