<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore } from '@/stores/settings'
import { useEnginesStore } from '@/stores/engines'
import { getAsrModels } from '@/config/providers'
import type { AsrModelOption, Provider } from '@/stores/types'
import { Input } from '@/components/ui/input'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import SegmentedToggle from '@/components/SegmentedToggle.vue'
import { formatRam } from '@/utils/format'
import { RefreshCw, Loader2 } from 'lucide-vue-next'

const { t } = useI18n()
const settings = useSettingsStore()
const engines = useEnginesStore()

const CUSTOM_MODEL_VALUE = '_custom'

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

const isCustomAsrModel = computed(() => {
  if (asrModelOptions.value.length === 0) return true
  return !asrModelOptions.value.includes(settings.asrCloudModel)
})

const asrModelSelectValue = computed(() => {
  if (asrModelOptions.value.length === 0) return CUSTOM_MODEL_VALUE
  if (asrModelOptions.value.includes(settings.asrCloudModel)) return settings.asrCloudModel
  return CUSTOM_MODEL_VALUE
})

async function onAsrModelSelect(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  if (value === CUSTOM_MODEL_VALUE) {
    await settings.setSetting('asr_cloud_model', '')
    return
  }
  await settings.setSetting('asr_cloud_model', value)
}

let asrModelDebounce: ReturnType<typeof setTimeout> | null = null

function onAsrModelInput(event: Event) {
  const value = (event.target as HTMLInputElement).value
  settings.asrCloudModel = value
  if (asrModelDebounce) clearTimeout(asrModelDebounce)
  asrModelDebounce = setTimeout(() => {
    settings.setSetting('asr_cloud_model', value)
  }, 500)
}

const refreshingAsr = ref(false)

async function refreshModels(provider: Provider | undefined, loadingRef: { value: boolean }) {
  if (!provider || loadingRef.value) return
  loadingRef.value = true
  try {
    const models = await invoke<string[]>('fetch_provider_models', { provider })
    await engines.updateProvider({ ...provider, cached_models: models })
  } catch (e) {
    console.error('refreshModels failed:', e)
  } finally {
    loadingRef.value = false
  }
}

function refreshAsrModels() {
  refreshModels(asrSelectedProvider.value, refreshingAsr)
}

const selectedAsrModel = computed(() =>
  engines.asrModels.find(m => m.id === settings.selectedModelId) ?? null
)

const asrGroupLabel = (group: AsrModelOption['group']) => t(`settings.asrGroup.${group}`)
const asrGroupClass = (group: AsrModelOption['group']) => {
  switch (group) {
    case 'local': return 'bg-blue-500/10 text-blue-600'
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

function werBadge(wer: number) {
  if (wer < 3) return { label: t('benchmark.wer.excellent'), cls: 'bg-emerald-500/10 text-emerald-600' }
  if (wer < 5) return { label: t('benchmark.wer.good'), cls: 'bg-blue-500/10 text-blue-600' }
  if (wer < 8) return { label: t('benchmark.wer.fair'), cls: 'bg-amber-500/10 text-amber-600' }
  return { label: t('benchmark.wer.basic'), cls: 'bg-orange-500/10 text-orange-600' }
}

function rtfBadge(rtf: number) {
  if (rtf < 0.05) return { label: t('benchmark.rtf.lightning'), cls: 'bg-violet-500/10 text-violet-600' }
  if (rtf < 0.15) return { label: t('benchmark.rtf.fast'), cls: 'bg-emerald-500/10 text-emerald-600' }
  if (rtf < 0.35) return { label: t('benchmark.rtf.normal'), cls: 'bg-blue-500/10 text-blue-600' }
  return { label: t('benchmark.rtf.slow'), cls: 'bg-amber-500/10 text-amber-600' }
}
</script>

<template>
  <div>
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
            <span v-if="selectedAsrModel" class="inline-flex items-center gap-1.5 truncate">
              <span class="truncate">{{ selectedAsrModel.label }}</span>
              <Badge
                variant="secondary"
                :class="['text-[9px] px-1 py-0 border-transparent font-medium shrink-0', asrGroupClass(selectedAsrModel.group)]"
              >{{ asrGroupLabel(selectedAsrModel.group) }}</Badge>
            </span>
          </SelectTrigger>
          <SelectContent>
            <SelectItem v-for="m in engines.asrModels" :key="m.id" :value="m.id">
              <div class="flex flex-col gap-0.5">
                <span class="flex items-center gap-1.5">
                  {{ m.label }}
                  <Badge v-if="m.recommended" variant="secondary" class="text-[9px] px-1 py-0 bg-emerald-500/10 text-emerald-600 border-transparent font-medium">{{ t('settings.cleanup.recommended') }}</Badge>
                  <Badge variant="secondary" :class="['text-[9px] px-1 py-0 border-transparent font-medium', asrGroupClass(m.group)]">{{ asrGroupLabel(m.group) }}</Badge>
                </span>
                <span v-if="m.wer != null || m.rtf != null || m.params != null || m.ram != null || (m.lang_codes && m.lang_codes.length > 0)" class="inline-flex items-center gap-1 flex-wrap">
                  <Badge v-if="m.wer != null" variant="secondary" :class="['text-[9px] px-1 py-0 border-transparent font-medium', werBadge(m.wer).cls]">{{ werBadge(m.wer).label }} <span class="opacity-50 font-normal">{{ +m.wer.toFixed(1) }}%</span></Badge>
                  <Badge v-if="m.rtf != null" variant="secondary" :class="['text-[9px] px-1 py-0 border-transparent font-medium', rtfBadge(m.rtf).cls]">{{ rtfBadge(m.rtf).label }} <span class="opacity-50 font-normal">{{ +m.rtf.toFixed(2) }}x</span></Badge>
                  <Badge v-if="m.params != null" variant="secondary" class="text-[9px] px-1 py-0 bg-slate-500/10 text-slate-600 border-transparent font-medium">{{ formatParams(m.params) }}</Badge>
                  <Badge v-if="m.ram != null" variant="secondary" class="text-[9px] px-1 py-0 bg-cyan-500/10 text-cyan-600 border-transparent font-medium">RAM <span class="opacity-50 font-normal">~{{ formatRam(m.ram) }}</span></Badge>
                  <Badge v-if="m.lang_codes && m.lang_codes.length > 0" variant="secondary" class="text-[9px] px-1 py-0 bg-indigo-500/10 text-indigo-600 border-transparent font-medium">{{ formatLangs(m.lang_codes) }}</Badge>
                </span>
              </div>
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
          <div class="flex items-center gap-2">
            <Select v-if="asrModelOptions.length > 0 && !isCustomAsrModel" :model-value="asrModelSelectValue" @update:model-value="onAsrModelSelect">
              <SelectTrigger class="w-auto min-w-[140px] h-8 text-xs">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem v-for="m in asrModelOptions" :key="m" :value="m">{{ m }}</SelectItem>
                <SelectItem :value="CUSTOM_MODEL_VALUE">{{ t('settings.cloudAsr.custom') }}</SelectItem>
              </SelectContent>
            </Select>
            <Input
              v-if="isCustomAsrModel"
              :value="settings.asrCloudModel"
              @input="onAsrModelInput"
              :placeholder="t('settings.cloudAsr.customPlaceholder')"
              class="h-8 text-xs min-w-[140px]"
            />
            <TooltipProvider :delay-duration="300">
              <Tooltip>
                <TooltipTrigger as-child>
                  <Button variant="outline" size="icon" class="h-8 w-8 shrink-0" :disabled="refreshingAsr" @click="refreshAsrModels">
                    <Loader2 v-if="refreshingAsr" class="w-3.5 h-3.5 animate-spin" />
                    <RefreshCw v-else class="w-3.5 h-3.5" />
                  </Button>
                </TooltipTrigger>
                <TooltipContent side="bottom" :side-offset="4">{{ t('settings.models.refresh') }}</TooltipContent>
              </Tooltip>
            </TooltipProvider>
          </div>
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
