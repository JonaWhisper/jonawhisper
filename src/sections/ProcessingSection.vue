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
import { Slider } from '@/components/ui/slider'
import {
  Select, SelectContent, SelectItem, SelectTrigger,
} from '@/components/ui/select'
import CloudModelPicker from '@/components/CloudModelPicker.vue'
import { Cpu, Cloud, Type, SpellCheck, MessageSquare } from 'lucide-vue-next'
import type { Component } from 'vue'

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

// Icon + color maps
const CLEANUP_TYPE_ICON: Record<CleanupModel['group'], Component> = {
  bert: Type,
  correction: SpellCheck,
  llm: MessageSquare,
  cloud: MessageSquare,
}
const CLEANUP_TYPE_COLOR: Record<CleanupModel['group'], string> = {
  bert: 'text-violet-500',
  correction: 'text-amber-500',
  llm: 'text-blue-500',
  cloud: 'text-sky-500',
}

function cleanupTypeIcon(group: CleanupModel['group']): Component {
  return CLEANUP_TYPE_ICON[group]
}
function cleanupTypeColor(group: CleanupModel['group']): string {
  return CLEANUP_TYPE_COLOR[group]
}
function isCloudCleanup(group: CleanupModel['group']): boolean {
  return group === 'cloud'
}

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
  settings.llmModel = value
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
    <div class="section-title">{{ t('panel.processing') }}</div>

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

      <!-- Unified cleanup dropdown with optgroups -->
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
              <component :is="cleanupTypeIcon(selectedCleanupModel.group)" :class="['w-3 h-3 shrink-0', cleanupTypeColor(selectedCleanupModel.group)]" />
              <span class="truncate">{{ selectedCleanupModel.label }}</span>
              <component :is="isCloudCleanup(selectedCleanupModel.group) ? Cloud : Cpu" :class="['w-3 h-3 shrink-0', isCloudCleanup(selectedCleanupModel.group) ? 'text-sky-500' : 'text-blue-500']" />
            </span>
          </SelectTrigger>
          <SelectContent>
            <SelectItem :value="DISABLED_VALUE">{{ t('settings.shortcut.cancel.none') }}</SelectItem>
            <SelectItem v-for="m in engines.cleanupModels" :key="m.id" :value="m.id">
              <span class="flex items-center gap-1.5">
                <component :is="cleanupTypeIcon(m.group)" :class="['w-3 h-3 shrink-0', cleanupTypeColor(m.group)]" />
                {{ m.label }}
                <Badge v-if="m.recommended" variant="secondary" class="text-[9px] px-1 py-0 bg-emerald-500/10 text-emerald-600 border-transparent font-medium">{{ t('settings.cleanup.recommended') }}</Badge>
                <component :is="isCloudCleanup(m.group) ? Cloud : Cpu" :class="['w-3 h-3 shrink-0 ml-auto', isCloudCleanup(m.group) ? 'text-sky-500' : 'text-blue-500']" />
              </span>
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
          <CloudModelPicker
            :model-options="llmModelOptions"
            :model-value="settings.llmModel"
            :refreshing="refreshingLlm"
            @update:model-value="onLlmModelChange"
            @refresh="refreshLlmModels"
          />
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
