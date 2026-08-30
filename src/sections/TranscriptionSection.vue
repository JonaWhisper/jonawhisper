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
import SettingToggle from '@/components/SettingToggle.vue'
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

function onToggle(v: boolean, key: string) {
  settings.setSetting(key, String(v))
}

/** Empty means "reuse the transcription model" — the preview is thrown away anyway. */
async function onPreviewModelChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('live_preview_model_id', value === SAME_AS_ASR ? '' : value)
}

async function onPreviewMaxLinesChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('live_preview_max_lines', value)
}

const LINE_CHOICES = ['1', '2', '3', '4', '5', '6', '8', '10']

const SAME_AS_ASR = '__same__'

const previewModelValue = computed(() => settings.livePreviewModelId || SAME_AS_ASR)

const localAsrModels = computed(() => engines.asrModels.filter(m => m.group === 'local'))

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
      <TriangleAlert class="w-4 h-4 text-amber-500 shrink-0 mt-0.5" />
      <div class="flex-1 min-w-0">
        <p class="text-xs text-amber-700 dark:text-amber-300">{{ t('settings.transcription.noModelWarning') }}</p>
        <button class="text-xs font-medium text-amber-600 dark:text-amber-400 hover:underline mt-1 cursor-pointer" @click="emit('navigate', 'models')">
          {{ t('settings.transcription.goToModels') }}
        </button>
      </div>
    </div>

    <!-- Cloud is available but nothing local: a choice to make, not a failure -->
    <div v-else-if="cloudOnly" class="flex items-start gap-2.5 rounded-xl border border-panel-card-border bg-panel-card-bg p-3 mb-2.5">
      <Cloud class="w-4 h-4 text-muted-foreground shrink-0 mt-0.5" />
      <div class="flex-1 min-w-0">
        <p class="text-xs text-muted-foreground">{{ t('settings.transcription.cloudOnlyNotice') }}</p>
        <button class="text-xs font-medium text-panel-accent hover:underline mt-1 cursor-pointer" @click="emit('navigate', 'models')">
          {{ t('settings.transcription.goToModels') }}
        </button>
      </div>
    </div>

    <!-- Speech recognition card -->
    <div class="bg-panel-card-bg backdrop-blur-sm border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
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
          <TriangleAlert class="w-3.5 h-3.5 shrink-0" />
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
    <div class="bg-panel-card-bg backdrop-blur-sm border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5" :class="{ 'opacity-35 pointer-events-none': engines.isCloudAsr }">
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

    <!-- Live preview -->
    <div class="bg-panel-card-bg backdrop-blur-sm border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">
        {{ t('settings.transcription.livePreview') }}
      </div>

      <SettingToggle
        setting-key="live_preview_enabled"
        :model-value="settings.livePreviewEnabled"
        :label="t('settings.transcription.livePreviewLabel')"
        :border-top="false"
        @update:model-value="onToggle"
      />

      <div v-if="settings.livePreviewEnabled" class="flex items-center justify-between gap-3 pt-2.5 mt-2.5 border-t border-panel-divider">
        <div class="min-w-0">
          <div class="text-[13px]">{{ t('settings.transcription.livePreviewModel') }}</div>
          <div class="text-[11px] text-muted-foreground">{{ t('settings.transcription.livePreviewHint') }}</div>
        </div>
        <Select :model-value="previewModelValue" @update:model-value="onPreviewModelChange">
          <SelectTrigger class="w-auto min-w-[180px] h-8 text-xs shrink-0">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem :value="SAME_AS_ASR">{{ t('settings.transcription.livePreviewSame') }}</SelectItem>
            <SelectItem v-for="m in localAsrModels" :key="m.id" :value="m.id">
              <ModelOption :label="m.label" location="local" />
            </SelectItem>
          </SelectContent>
        </Select>
      </div>

      <div v-if="settings.livePreviewEnabled" class="flex items-center justify-between gap-3 pt-2.5 mt-2.5 border-t border-panel-divider">
        <div class="min-w-0">
          <div class="text-[13px]">{{ t('settings.transcription.livePreviewMaxLines') }}</div>
          <div class="text-[11px] text-muted-foreground">{{ t('settings.transcription.livePreviewMaxLinesHint') }}</div>
        </div>
        <Select :model-value="String(settings.livePreviewMaxLines)" @update:model-value="onPreviewMaxLinesChange">
          <SelectTrigger class="w-auto min-w-[90px] h-8 text-xs shrink-0">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="n in LINE_CHOICES" :key="n" :value="n">{{ n }}</SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>
  </div>
</template>
