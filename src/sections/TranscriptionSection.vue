<script setup lang="ts">
import { computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/settings'
import { useEnginesStore } from '@/stores/engines'
import { getAsrModels } from '@/config/providers'
import { useProviderModels } from '@/composables/useProviderModels'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import SegmentedToggle from '@/components/SegmentedToggle.vue'
import CloudModelPicker from '@/components/CloudModelPicker.vue'
import ModelOption from '@/components/ModelOption.vue'
import { TriangleAlert, Cloud } from 'lucide-vue-next'

const { t } = useI18n()
const settings = useSettingsStore()
const engines = useEnginesStore()

const emit = defineEmits<{
  'navigate': [section: string]
}>()

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

const {
  selectedProvider: asrSelectedProvider,
  modelOptions: asrModelOptions,
  refreshing: refreshingAsr,
  refreshModels: refreshAsrModels,
} = useProviderModels(() => engines.asrCloudProviderId, getAsrModels)

async function onAsrCloudModelChange(value: string) {
  await settings.setSetting('asr_cloud_model', value)
}

const hasLocalAsr = computed(() => engines.asrModels.some(m => m.group === 'local'))
const cloudOnly = computed(() => !hasLocalAsr.value && engines.asrModels.length > 0)

const selectedAsrModel = computed(() =>
  engines.asrModels.find(m => m.id === settings.selectedModelId) ?? null
)

onMounted(() => {
  if (engines.languages.length === 0) {
    engines.fetchLanguages()
  }
})

</script>

<template>
  <div>
    <div class="text-[20px] font-bold tracking-[-0.02em] mb-4">{{ t('panel.transcription') }}</div>

    <!-- Nothing available at all: transcription cannot run -->
    <div v-if="engines.asrModels.length === 0" class="flex items-start gap-2.5 rounded-xl border border-amber-500/30 bg-amber-500/8 p-3 mb-2.5">
      <TriangleAlert class="w-4 h-4 text-amber-500 flex-shrink-0 mt-0.5" />
      <div class="flex-1 min-w-0">
        <p class="text-xs text-amber-700 dark:text-amber-300">{{ t('settings.transcription.noModelWarning') }}</p>
        <button class="text-xs font-medium text-amber-600 dark:text-amber-400 hover:underline mt-1 cursor-pointer" @click="emit('navigate', 'models')">
          {{ t('settings.transcription.goToModels') }}
        </button>
      </div>
    </div>

    <!-- Cloud is available but nothing local: a choice to make, not a failure -->
    <div v-else-if="cloudOnly" class="flex items-start gap-2.5 rounded-xl border border-panel-card-border bg-panel-card-bg p-3 mb-2.5">
      <Cloud class="w-4 h-4 text-muted-foreground flex-shrink-0 mt-0.5" />
      <div class="flex-1 min-w-0">
        <p class="text-xs text-muted-foreground">{{ t('settings.transcription.cloudOnlyNotice') }}</p>
        <button class="text-xs font-medium text-panel-accent hover:underline mt-1 cursor-pointer" @click="emit('navigate', 'models')">
          {{ t('settings.transcription.goToModels') }}
        </button>
      </div>
    </div>

    <!-- Speech recognition card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.transcription.model') }}</div>

      <!-- Model selector row -->
      <div class="flex items-center justify-between py-2 gap-3">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.transcription.model') }}</div>
        </div>
        <!-- Single model: display inline, no dropdown -->
        <div v-if="engines.asrModels.length === 1 && selectedAsrModel" class="flex h-8 items-center px-3 text-xs">
          <ModelOption
            :label="selectedAsrModel.label"
            :location="selectedAsrModel.group === 'cloud' ? 'cloud' : 'local'"
            compact
          />
        </div>
        <!-- Multiple models: full dropdown -->
        <Select
          v-else-if="engines.asrModels.length > 0"
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
            <span v-else class="text-muted-foreground">{{ t('settings.shortcut.cancel.none') }}</span>
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
        <!-- No models: warning -->
        <div v-else class="flex h-8 items-center rounded-md border border-amber-500/30 bg-amber-500/10 px-3 text-xs text-amber-600 dark:text-amber-400 min-w-[180px] gap-1.5">
          <TriangleAlert class="w-3.5 h-3.5 flex-shrink-0" />
          {{ t('settings.transcription.noModels') }}
        </div>
      </div>

      <!-- Cloud ASR sub-settings (model + refresh) -->
      <template v-if="engines.isCloudAsr && asrSelectedProvider">
        <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
          <div>
            <div class="text-[13px] text-foreground">{{ t('settings.cloudAsr.model') }}</div>
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
      <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.transcription.language') }}</div>
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
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5" :class="{ 'opacity-35 pointer-events-none': engines.isCloudAsr }">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.transcription.gpuMode') }}</div>
      <div class="flex items-center justify-between py-2 gap-3">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.transcription.gpuMode') }}</div>
        </div>
        <SegmentedToggle
          :model-value="settings.gpuMode"
          :options="[
            { value: 'auto', label: t('settings.transcription.gpuMode.auto'), badge: t('settings.cleanup.recommended') },
            { value: 'gpu', label: t('settings.transcription.gpuMode.gpu') },
            { value: 'cpu', label: t('settings.transcription.gpuMode.cpu') },
          ]"
          @update:model-value="onGpuModeChange"
        />
      </div>
    </div>
  </div>
</template>
