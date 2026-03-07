<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore } from '@/stores/settings'
import { useEnginesStore } from '@/stores/engines'
import { getLlmModels } from '@/config/providers'
import { Switch } from '@/components/ui/switch'
import { Slider } from '@/components/ui/slider'
import {
  Select, SelectContent, SelectItem, SelectTrigger,
} from '@/components/ui/select'
import CloudModelPicker from '@/components/CloudModelPicker.vue'
import ModelOption from '@/components/ModelOption.vue'

const { t } = useI18n()
const settings = useSettingsStore()
const engines = useEnginesStore()

const DISABLED_VALUE = '_disabled'

// --- Punctuation dropdown ---
const punctuationValue = computed(() => settings.punctuationModelId || DISABLED_VALUE)

async function onPunctuationChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('punctuation_model_id', value === DISABLED_VALUE ? '' : value)
}

const selectedPunctuationModel = computed(() =>
  engines.punctuationModels.find(m => m.id === settings.punctuationModelId) ?? null
)

// --- Cleanup dropdown (correction / LLM) ---
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

// LLM config (shown when cloud cleanup selected)
const llmSelectedProvider = computed(() =>
  engines.providers.find(p => p.id === engines.cleanupCloudProviderId)
)

const llmModelOptions = computed(() => {
  const provider = llmSelectedProvider.value
  return provider ? getLlmModels(provider) : []
})

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

async function onLlmModelChange(value: string) {
  await settings.setSetting('llm_model', value)
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
    <div class="text-[20px] font-bold tracking-[-0.02em] mb-4">{{ t('panel.processing') }}</div>

    <!-- Pre-processing card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.postProcessing.vad') }}</div>
      <div class="flex items-center justify-between py-2 gap-3">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.postProcessing.vad') }}</div>
        </div>
        <Switch
          :model-value="settings.vadEnabled"
          @update:model-value="(v: boolean) => settings.setSetting('vad_enabled', String(v))"
        />
      </div>
    </div>

    <!-- Post-processing card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.postProcessing.textCleanup') }}</div>

      <!-- Hallucination filter -->
      <div class="flex items-center justify-between py-2 gap-3">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.postProcessing.hallucinations') }}</div>
        </div>
        <Switch
          :model-value="settings.hallucinationFilterEnabled"
          @update:model-value="(v: boolean) => settings.setSetting('hallucination_filter_enabled', String(v))"
        />
      </div>

      <!-- Disfluency removal -->
      <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.postProcessing.disfluencyRemoval') }}</div>
        </div>
        <Switch
          :model-value="settings.disfluencyRemovalEnabled"
          @update:model-value="(v: boolean) => settings.setSetting('disfluency_removal_enabled', String(v))"
        />
      </div>

      <!-- ITN (Inverse Text Normalization) -->
      <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.postProcessing.itn') }}</div>
        </div>
        <Switch
          :model-value="settings.itnEnabled"
          @update:model-value="(v: boolean) => settings.setSetting('itn_enabled', String(v))"
        />
      </div>

      <!-- Spell-check -->
      <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.postProcessing.spellcheck') }}</div>
        </div>
        <Switch
          :model-value="settings.spellcheckEnabled"
          @update:model-value="(v: boolean) => settings.setSetting('spellcheck_enabled', String(v))"
        />
      </div>

      <!-- Punctuation dropdown -->
      <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.postProcessing.punctuation') }}</div>
        </div>
        <Select
          :model-value="punctuationValue"
          @update:model-value="onPunctuationChange"
        >
          <SelectTrigger class="w-auto min-w-[190px] h-8 text-xs">
            <span v-if="punctuationValue === DISABLED_VALUE" class="text-muted-foreground">
              {{ t('settings.cleanup.disabled') }}
            </span>
            <ModelOption
              v-else-if="selectedPunctuationModel"
              :label="selectedPunctuationModel.label"
              type="punctuation"
              location="local"
              compact
            />
          </SelectTrigger>
          <SelectContent>
            <SelectItem :value="DISABLED_VALUE">{{ t('settings.cleanup.disabled') }}</SelectItem>
            <SelectItem v-for="m in engines.punctuationModels" :key="m.id" :value="m.id">
              <ModelOption
                :label="m.label"
                type="punctuation"
                location="local"
                :recommended="m.recommended"
              />
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <!-- Text cleanup dropdown (correction / LLM) -->
      <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.postProcessing.textCleanup') }}</div>
        </div>
        <Select
          :model-value="unifiedCleanupValue"
          @update:model-value="onUnifiedCleanupChange"
        >
          <SelectTrigger class="w-auto min-w-[190px] h-8 text-xs">
            <span v-if="unifiedCleanupValue === DISABLED_VALUE" class="text-muted-foreground">
              {{ t('settings.cleanup.disabled') }}
            </span>
            <ModelOption
              v-else-if="selectedCleanupModel"
              :label="selectedCleanupModel.label"
              :type="selectedCleanupModel.group === 'cloud' ? 'llm' : selectedCleanupModel.group as any"
              :location="selectedCleanupModel.group === 'cloud' ? 'cloud' : 'local'"
              compact
            />
          </SelectTrigger>
          <SelectContent>
            <SelectItem :value="DISABLED_VALUE">{{ t('settings.cleanup.disabled') }}</SelectItem>
            <SelectItem v-for="m in engines.cleanupModels" :key="m.id" :value="m.id">
              <ModelOption
                :label="m.label"
                :type="m.group === 'cloud' ? 'llm' : m.group as any"
                :location="m.group === 'cloud' ? 'cloud' : 'local'"
                :recommended="m.recommended"
              />
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>

    <!-- Cloud LLM sub-settings -->
    <template v-if="settings.textCleanupEnabled && engines.isCloudLlm && llmSelectedProvider">
      <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
        <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.llm.model') }}</div>
        <div class="flex items-center justify-between py-2 gap-3">
          <div>
            <div class="text-[13px] text-foreground">{{ t('settings.llm.model') }}</div>
          </div>
          <CloudModelPicker
            :model-options="llmModelOptions"
            :model-value="settings.llmModel"
            :refreshing="refreshingLlm"
            @update:model-value="onLlmModelChange"
            @refresh="refreshLlmModels"
          />
        </div>
        <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
          <div>
            <div class="text-[13px] text-foreground">{{ t('settings.llm.maxTokens') }}</div>
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
      <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
        <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.llm.maxTokens') }}</div>
        <div class="flex items-center justify-between py-2 gap-3">
          <div>
            <div class="text-[13px] text-foreground">{{ t('settings.llm.maxTokens') }}</div>
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
