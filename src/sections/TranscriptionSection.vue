<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore } from '@/stores/settings'
import { useEnginesStore } from '@/stores/engines'
import { getAsrModels } from '@/config/providers'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import SegmentedToggle from '@/components/SegmentedToggle.vue'
import CloudModelPicker from '@/components/CloudModelPicker.vue'
import ModelOption from '@/components/ModelOption.vue'

const { t } = useI18n()
const settings = useSettingsStore()
const engines = useEnginesStore()

async function onAsrModelChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('selected_model_id', value)
}

async function onLanguageChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('selected_language', value)
}

async function onGpuModeChange(mode: string) {
  await settings.setSetting('gpu_mode', mode)
}

const asrSelectedProvider = computed(() =>
  engines.providers.find(p => p.id === engines.asrCloudProviderId)
)

const asrModelOptions = computed(() => {
  const provider = asrSelectedProvider.value
  return provider ? getAsrModels(provider) : []
})

const refreshingAsr = ref(false)

async function refreshAsrModels() {
  const provider = asrSelectedProvider.value
  if (!provider || refreshingAsr.value) return
  refreshingAsr.value = true
  try {
    const models = await invoke<string[]>('fetch_provider_models', { provider })
    await engines.updateProvider({ ...provider, cached_models: models })
  } catch (e) {
    console.error('refreshModels failed:', e)
  } finally {
    refreshingAsr.value = false
  }
}

async function onAsrCloudModelChange(value: string) {
  settings.asrCloudModel = value
  await settings.setSetting('asr_cloud_model', value)
}

const selectedAsrModel = computed(() =>
  engines.asrModels.find(m => m.id === settings.selectedModelId) ?? null
)

</script>

<template>
  <div>
    <div class="section-title">{{ t('panel.transcription') }}</div>

    <!-- Speech recognition card -->
    <div class="wf-card">
      <div class="wf-card-title">{{ t('settings.transcription.model') }}</div>

      <!-- Model selector row -->
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('settings.transcription.model') }}</div>
        </div>
        <Select
          v-if="engines.asrModels.length > 0"
          :model-value="settings.selectedModelId"
          @update:model-value="onAsrModelChange"
        >
          <SelectTrigger class="w-auto min-w-[180px] h-8 text-xs">
            <ModelOption
              v-if="selectedAsrModel"
              :label="selectedAsrModel.label"
              :location="selectedAsrModel.group === 'cloud' ? 'cloud' : 'local'"
              compact
            />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="m in engines.asrModels" :key="m.id" :value="m.id">
              <ModelOption
                :label="m.label"
                :location="m.group === 'cloud' ? 'cloud' : 'local'"
                :recommended="m.recommended"
              />
            </SelectItem>
          </SelectContent>
        </Select>
        <p v-else class="text-xs text-muted-foreground">
          {{ t('settings.transcription.noModels') }}
        </p>
      </div>

      <!-- Cloud ASR sub-settings (model + refresh) -->
      <template v-if="engines.isCloudAsr && asrSelectedProvider">
        <div class="wf-form-row">
          <div>
            <div class="wf-form-label">{{ t('settings.cloudAsr.model') }}</div>
          </div>
          <CloudModelPicker
            :model-options="asrModelOptions"
            :model-value="settings.asrCloudModel"
            :refreshing="refreshingAsr"
            @update:model-value="onAsrCloudModelChange"
            @refresh="refreshAsrModels"
          />
        </div>
      </template>

      <!-- Language -->
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('settings.transcription.language') }}</div>
        </div>
        <Select :model-value="settings.selectedLanguage" @update:model-value="onLanguageChange">
          <SelectTrigger class="w-auto min-w-[120px] h-8 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="lang in engines.languages" :key="lang.code" :value="lang.code">
              {{ lang.label }}
            </SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>

    <!-- GPU Acceleration card (grayed when cloud ASR) -->
    <div class="wf-card" :class="{ 'opacity-35 pointer-events-none': engines.isCloudAsr }">
      <div class="wf-card-title">{{ t('settings.transcription.gpuMode') }}</div>
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('settings.transcription.gpuMode') }}</div>
        </div>
        <SegmentedToggle
          :model-value="settings.gpuMode"
          :options="[
            { value: 'auto', label: t('settings.transcription.gpuMode.auto') },
            { value: 'gpu', label: t('settings.transcription.gpuMode.gpu') },
            { value: 'cpu', label: t('settings.transcription.gpuMode.cpu') },
          ]"
          @update:model-value="onGpuModeChange"
        />
      </div>
    </div>
  </div>
</template>
