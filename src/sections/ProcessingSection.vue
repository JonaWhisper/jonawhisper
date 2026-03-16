<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/settings'
import { useEnginesStore } from '@/stores/engines'
import { getLlmModels } from '@/config/providers'
import {
  Select, SelectContent, SelectItem, SelectTrigger,
} from '@/components/ui/select'
import CloudModelPicker from '@/components/CloudModelPicker.vue'
import ModelOption from '@/components/ModelOption.vue'
import MaxTokensSlider from '@/components/MaxTokensSlider.vue'
import SettingToggle from '@/components/SettingToggle.vue'
import { useProviderModels } from '@/composables/useProviderModels'
import { BookOpen } from 'lucide-vue-next'

const emit = defineEmits<{
  'navigate': [section: string]
}>()

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
const {
  selectedProvider: llmSelectedProvider,
  modelOptions: llmModelOptions,
  refreshing: refreshingLlm,
  refreshModels: refreshLlmModels,
} = useProviderModels(() => engines.cleanupCloudProviderId, getLlmModels)

async function onLlmModelChange(value: string) {
  await settings.setSetting('llm_model', value)
}

function onToggle(v: boolean, key: string) {
  settings.setSetting(key, String(v))
}
</script>

<template>
  <div>
    <div class="text-[20px] font-bold tracking-[-0.02em] mb-4">{{ t('panel.processing') }}</div>

    <!-- Pre-processing card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.postProcessing.vad') }}</div>
      <SettingToggle
        setting-key="vad_enabled"
        :model-value="settings.vadEnabled"
        :label="t('settings.postProcessing.vad')"
        :border-top="false"
        @update:model-value="onToggle"
      />
    </div>

    <!-- Post-processing card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.postProcessing.textCleanup') }}</div>

      <!-- Hallucination filter -->
      <SettingToggle
        setting-key="hallucination_filter_enabled"
        :model-value="settings.hallucinationFilterEnabled"
        :label="t('settings.postProcessing.hallucinations')"
        :border-top="false"
        @update:model-value="onToggle"
      />

      <!-- Disfluency removal -->
      <SettingToggle
        setting-key="disfluency_removal_enabled"
        :model-value="settings.disfluencyRemovalEnabled"
        :label="t('settings.postProcessing.disfluencyRemoval')"
        @update:model-value="onToggle"
      />

      <!-- ITN (Inverse Text Normalization) -->
      <SettingToggle
        setting-key="itn_enabled"
        :model-value="settings.itnEnabled"
        :label="t('settings.postProcessing.itn')"
        @update:model-value="onToggle"
      />

      <!-- Spell-check -->
      <SettingToggle
        setting-key="spellcheck_enabled"
        :model-value="settings.spellcheckEnabled"
        :label="t('settings.postProcessing.spellcheck')"
        :description="!engines.hasSpellcheckDict ? t('settings.postProcessing.spellcheckNoDict') : undefined"
        :disabled="!engines.hasSpellcheckDict"
        @update:model-value="onToggle"
      />

      <!-- User dictionary -->
      <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.postProcessing.userDict') }}</div>
          <div class="text-[11px] text-muted-foreground">{{ t('settings.postProcessing.userDictDescription') }}</div>
        </div>
        <button
          class="inline-flex items-center gap-1.5 rounded-md border border-input bg-background h-8 px-3 text-xs hover:bg-accent hover:text-accent-foreground shrink-0"
          @click="emit('navigate', 'dictionary')"
        >
          <BookOpen class="h-3.5 w-3.5" />
          <span>{{ t('settings.postProcessing.userDictOpen') }}</span>
        </button>
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
        <MaxTokensSlider class="border-t-[0.5px] border-panel-divider" />
      </div>
    </template>

    <!-- Local LLM sub-settings (token cap only) -->
    <template v-if="settings.textCleanupEnabled && engines.isLocalLlm">
      <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
        <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.llm.maxTokens') }}</div>
        <MaxTokensSlider />
      </div>
    </template>
  </div>
</template>
