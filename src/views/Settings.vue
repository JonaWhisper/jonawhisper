<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useSettingsStore } from '@/stores/settings'
import { useEnginesStore } from '@/stores/engines'
import type { Provider } from '@/stores/types'
import { getAsrModels, getLlmModels } from '@/config/providers'
import { Settings, Cloud, Sparkles, Keyboard, Mic, AudioLines, Laptop, Usb, Bluetooth, Waves, HardDrive, Zap, Monitor, Pencil, Trash2, Plus, RefreshCw, Loader2 } from 'lucide-vue-next'
import type { Component } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { i18n } from '@/main'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Slider } from '@/components/ui/slider'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import SpectrumBars from '@/components/SpectrumBars.vue'
import ShortcutCapture from '@/components/ShortcutCapture.vue'
import SegmentedToggle from '@/components/SegmentedToggle.vue'
import ProviderForm from '@/components/ProviderForm.vue'
import { serializeShortcut } from '@/utils/shortcut'
import { Badge } from '@/components/ui/badge'
import { formatRam } from '@/utils/format'
import type { CleanupModel, AsrModelOption } from '@/stores/types'

const { t } = useI18n()
const settings = useSettingsStore()
const engines = useEnginesStore()

// Active section
const activeSection = ref('general')

const sections = [
  { id: 'general', icon: Settings, label: 'settings.section.general' },
  { id: 'providers', icon: Cloud, label: 'settings.section.providers' },
  { id: 'transcription', icon: AudioLines, label: 'settings.section.transcription' },
  { id: 'postprocessing', icon: Sparkles, label: 'settings.section.postProcessing' },
  { id: 'shortcuts', icon: Keyboard, label: 'settings.section.shortcuts' },
  { id: 'microphone', icon: Mic, label: 'settings.section.microphone' },
]

// Mic test
const isTesting = ref(false)
const testSpectrum = ref<number[]>(new Array(12).fill(0))
let spectrumUnlisten: (() => void) | null = null

const localeOptions = [
  { value: 'auto', label: 'settings.locale.auto' },
  { value: 'fr', label: 'settings.locale.fr' },
  { value: 'en', label: 'settings.locale.en' },
]

async function onLocaleChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('app_locale', value)
  // Rust handles tray labels; resolve effective locale for frontend
  const locale = await invoke<string>('get_system_locale')
  i18n.global.locale.value = locale as 'fr' | 'en'
}

async function onHallucinationFilterChange(enabled: boolean) {
  await settings.setSetting('hallucination_filter_enabled', String(enabled))
}

async function onAudioDuckingChange(enabled: boolean) {
  await settings.setSetting('audio_ducking_enabled', String(enabled))
}

const duckingSliderValue = ref(settings.audioDuckingLevel * 100)
watch(() => settings.audioDuckingLevel, (v) => { duckingSliderValue.value = v * 100 })
function onDuckingSliderUpdate(v: number[] | undefined) {
  if (v?.[0] != null) duckingSliderValue.value = v[0]
}
function onDuckingSliderCommit(v: number[]) {
  const val = v[0] ?? duckingSliderValue.value
  settings.setSetting('audio_ducking_level', String(val / 100))
}

async function onHotkeyChange(value: string) {
  await settings.setSetting('hotkey', value)
}

async function onCancelShortcutChange(value: string) {
  await settings.setSetting('cancel_shortcut', value)
}

function onDisableCancel() {
  const disabled = serializeShortcut({ key_code: 0, modifiers: 0, kind: 'Key' })
  onCancelShortcutChange(disabled)
}

async function onRecordingModeChange(mode: string) {
  await settings.setSetting('recording_mode', mode)
}

const TRANSPORT_ICONS: Record<string, Component> = {
  BuiltIn: Laptop, USB: Usb, Bluetooth: Bluetooth,
  Virtual: Waves, Aggregate: HardDrive, Thunderbolt: Zap,
  HDMI: Monitor, Unknown: Mic,
}
function deviceIcon(type: string): Component { return TRANSPORT_ICONS[type] ?? Mic }

// Selected device UID: use the stored preference, or the default device UID
const selectedDeviceUid = computed(() => {
  const devices = engines.audioDevices
  const stored = devices.find(d => d.uid === settings.selectedInputDeviceUid)
  if (stored) return stored.uid
  const def = devices.find(d => d.is_default)
  return def?.uid ?? ''
})

const selectedDevice = computed(() =>
  engines.audioDevices.find(d => d.uid === selectedDeviceUid.value)
)

async function onDeviceChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  const defaultDevice = engines.audioDevices.find(d => d.is_default)
  const uid = (defaultDevice && value === defaultDevice.uid) ? '' : value
  await settings.setSetting('selected_input_device_uid', uid)
}

// -- Providers management --
const showAddForm = ref(false)
const addFormKey = ref(0)
const editingProviderIds = ref(new Set<string>())
const showRemoveConfirm = ref(false)
const removeTarget = ref<Provider | null>(null)

function startAddProvider() {
  addFormKey.value++
  showAddForm.value = true
}

function cancelAddProvider() {
  showAddForm.value = false
}

async function saveNewProvider(provider: Provider) {
  await engines.addProvider(provider)
  showAddForm.value = false
}

function startEditProvider(provider: Provider) {
  editingProviderIds.value.add(provider.id)
}

function cancelEditProvider(providerId: string) {
  editingProviderIds.value.delete(providerId)
}

async function saveEditedProvider(provider: Provider) {
  await engines.updateProvider(provider)
  editingProviderIds.value.delete(provider.id)
}

function requestRemoveProvider(provider: Provider) {
  removeTarget.value = provider
  showRemoveConfirm.value = true
}

async function confirmRemoveProvider() {
  if (removeTarget.value) {
    await engines.removeProvider(removeTarget.value.id)
  }
  showRemoveConfirm.value = false
  removeTarget.value = null
}

// -- Transcription --
const CUSTOM_MODEL_VALUE = '_custom'

async function onAsrModelChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('selected_model_id', value)
}

async function onLanguageChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('selected_language', value)
}

async function onGpuModeChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('gpu_mode', value)
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

// -- LLM config --

const llmSelectedProvider = computed(() =>
  engines.providers.find(p => p.id === engines.cleanupCloudProviderId)
)

const llmModelOptions = computed(() => {
  const provider = llmSelectedProvider.value
  return provider ? getLlmModels(provider) : []
})

const isCustomLlmModel = computed(() => llmModelOptions.value.length === 0)

// Refresh models from provider API
const refreshingAsr = ref(false)
const refreshingLlm = ref(false)

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

function refreshLlmModels() {
  refreshModels(llmSelectedProvider.value, refreshingLlm)
}

let llmModelDebounce: ReturnType<typeof setTimeout> | null = null

async function onTextCleanupChange(enabled: boolean) {
  await settings.setSetting('text_cleanup_enabled', String(enabled))
}

const selectedCleanupModel = computed(() => {
  return engines.cleanupModels.find(m => m.id === settings.cleanupModelId) ?? null
})

const cleanupGroupLabel = (group: CleanupModel['group']) => {
  const key = `settings.cleanupGroup.${group}`
  return t(key)
}

const cleanupGroupClass = (group: CleanupModel['group']) => {
  switch (group) {
    case 'bert': return 'bg-violet-500/10 text-violet-600'
    case 'correction': return 'bg-amber-500/10 text-amber-600'
    case 'llm': return 'bg-blue-500/10 text-blue-600'
    case 'cloud': return 'bg-sky-500/10 text-sky-600'
  }
}

const asrGroupLabel = (group: AsrModelOption['group']) => {
  return t(`settings.asrGroup.${group}`)
}

const asrGroupClass = (group: AsrModelOption['group']) => {
  switch (group) {
    case 'local': return 'bg-blue-500/10 text-blue-600'
    case 'cloud': return 'bg-sky-500/10 text-sky-600'
  }
}

const selectedAsrModel = computed(() => {
  return engines.asrModels.find(m => m.id === settings.selectedModelId) ?? null
})

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

async function onCleanupModelChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('cleanup_model_id', value)
}

function onMaxTokensSliderUpdate(v: number[] | undefined) {
  if (v?.[0] != null) settings.llmMaxTokens = v[0]
}
function onMaxTokensSliderCommit(v: number[]) {
  const val = v[0] ?? settings.llmMaxTokens
  settings.setSetting('llm_max_tokens', String(val))
}

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

async function startMicTest() {
  if (isTesting.value) return
  isTesting.value = true
  testSpectrum.value = new Array(12).fill(0)

  await invoke('start_mic_test')

  spectrumUnlisten = await listen<number[]>('mic-test-spectrum', (event) => {
    if (!isTesting.value) return
    const bands = event.payload
    const smoothed = [...testSpectrum.value]
    for (let i = 0; i < smoothed.length; i++) {
      const newVal = i < bands.length ? (bands[i] ?? 0) : 0
      smoothed[i] = (smoothed[i] ?? 0) * 0.45 + newVal * 0.55
    }
    testSpectrum.value = smoothed
  })
}

async function stopMicTest() {
  isTesting.value = false
  testSpectrum.value = new Array(12).fill(0)
  if (spectrumUnlisten) {
    spectrumUnlisten()
    spectrumUnlisten = null
  }
  await invoke('stop_mic_test')
}

let micTestStoppedUnlisten: (() => void) | null = null

onMounted(async () => {
  getCurrentWindow().setTitle(t('window.settings'))
  await Promise.all([
    settings.fetchSettings(),
    engines.fetchAudioDevices(),
    engines.fetchProviders(),
    engines.fetchEngines(),
    engines.fetchModels(),
    engines.fetchLanguages(),
  ])

  // Listen for mic test being auto-cancelled (e.g. recording started while testing)
  micTestStoppedUnlisten = await listen('mic-test-stopped', () => {
    isTesting.value = false
    testSpectrum.value = new Array(12).fill(0)
    if (spectrumUnlisten) {
      spectrumUnlisten()
      spectrumUnlisten = null
    }
  })
})

onUnmounted(() => {
  stopMicTest()
  if (micTestStoppedUnlisten) {
    micTestStoppedUnlisten()
    micTestStoppedUnlisten = null
  }
})
</script>

<template>
  <div class="flex h-full min-w-0 select-none">
    <!-- Sidebar -->
    <div class="w-48 min-w-[10rem] border-r border-border bg-muted/30 overflow-y-auto flex-shrink-0">
      <div class="p-3">
        <h2 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
          {{ t('settings.title') }}
        </h2>
      </div>
      <div class="space-y-1 px-1">
        <button
          v-for="section in sections"
          :key="section.id"
          @click="activeSection = section.id"
          class="w-full text-left px-3 py-1.5 rounded-md text-sm transition-colors"
          :class="activeSection === section.id
            ? 'bg-accent text-accent-foreground'
            : 'hover:bg-accent/50 text-foreground'"
        >
          <div class="flex items-center gap-2">
            <component :is="section.icon" class="w-4 h-4 flex-shrink-0" />
            <span class="font-medium truncate">{{ t(section.label) }}</span>
          </div>
        </button>
      </div>
    </div>

    <!-- Content -->
    <div class="flex-1 min-w-0 overflow-y-auto p-5">
      <!-- General -->
      <div v-if="activeSection === 'general'">
        <h2 class="text-lg font-semibold mb-4">{{ t('settings.section.general') }}</h2>

        <div class="space-y-4">
          <div class="space-y-2">
            <Label class="text-sm font-medium">{{ t('settings.locale') }}</Label>
            <Select :model-value="settings.appLocale" @update:model-value="onLocaleChange">
              <SelectTrigger class="w-full h-9 text-sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="opt in localeOptions"
                  :key="opt.value"
                  :value="opt.value"
                >
                  {{ t(opt.label) }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>
      </div>

      <!-- Providers -->
      <div v-if="activeSection === 'providers'">
        <div class="flex items-center justify-between mb-4">
          <h2 class="text-lg font-semibold">{{ t('settings.section.providers') }}</h2>
          <Button v-if="!showAddForm" variant="outline" size="sm" @click="startAddProvider">
            <Plus class="w-4 h-4 mr-1" />
            {{ t('settings.providers.add') }}
          </Button>
        </div>

        <div class="space-y-3">
          <div v-if="engines.providers.length === 0 && !showAddForm" class="text-sm text-muted-foreground">
            {{ t('settings.providers.empty') }}
          </div>

          <div v-for="provider in engines.providers" :key="provider.id" class="rounded-md border border-border">
            <!-- Edit mode: inline form -->
            <div v-if="editingProviderIds.has(provider.id)" class="p-4">
              <ProviderForm
                :provider="provider"
                @save="saveEditedProvider"
                @cancel="cancelEditProvider(provider.id)"
              />
            </div>
            <!-- Display mode -->
            <div v-else class="flex items-center gap-3 px-3 py-2">
              <div class="flex-1 min-w-0">
                <div class="text-sm font-medium truncate">{{ provider.name }}</div>
                <div v-if="provider.kind === 'Custom'" class="text-xs text-muted-foreground truncate">{{ provider.url }}</div>
              </div>
              <span class="text-xs px-1.5 py-0.5 rounded bg-muted text-muted-foreground shrink-0">{{ provider.kind }}</span>
              <Button variant="ghost" size="icon" class="h-7 w-7 shrink-0" @click="startEditProvider(provider)">
                <Pencil class="w-3.5 h-3.5" />
              </Button>
              <Button variant="ghost" size="icon" class="h-7 w-7 shrink-0 text-destructive hover:text-destructive" @click="requestRemoveProvider(provider)">
                <Trash2 class="w-3.5 h-3.5" />
              </Button>
            </div>
          </div>

          <!-- Add new provider form -->
          <div v-if="showAddForm" class="rounded-md border border-border p-4">
            <ProviderForm
              :key="addFormKey"
              @save="saveNewProvider"
              @cancel="cancelAddProvider"
            />
          </div>
        </div>
      </div>

      <!-- Transcription -->
      <div v-if="activeSection === 'transcription'">
        <h2 class="text-lg font-semibold mb-4">{{ t('settings.section.transcription') }}</h2>

        <div class="space-y-4">
          <!-- Unified model selector (local models + cloud providers) -->
          <div class="space-y-1">
            <Label class="text-sm font-medium">{{ t('settings.transcription.model') }}</Label>
            <Select
              v-if="engines.asrModels.length > 0"
              :model-value="settings.selectedModelId"
              @update:model-value="onAsrModelChange"
            >
              <SelectTrigger class="w-full h-9 text-sm">
                <span v-if="selectedAsrModel" class="inline-flex items-center gap-1.5 truncate">
                  <span class="truncate">{{ selectedAsrModel.label }}</span>
                  <Badge
                    variant="secondary"
                    :class="['text-[9px] px-1 py-0 border-transparent font-medium shrink-0', asrGroupClass(selectedAsrModel.group)]"
                  >{{ asrGroupLabel(selectedAsrModel.group) }}</Badge>
                </span>
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="m in engines.asrModels"
                  :key="m.id"
                  :value="m.id"
                >
                  <div class="flex flex-col gap-0.5">
                    <span class="flex items-center gap-1.5">
                      {{ m.label }}
                      <Badge
                        v-if="m.recommended"
                        variant="secondary"
                        class="text-[9px] px-1 py-0 bg-emerald-500/10 text-emerald-600 border-transparent font-medium"
                      >{{ t('settings.cleanup.recommended') }}</Badge>
                      <Badge
                        variant="secondary"
                        :class="['text-[9px] px-1 py-0 border-transparent font-medium', asrGroupClass(m.group)]"
                      >{{ asrGroupLabel(m.group) }}</Badge>
                    </span>
                    <span v-if="m.wer != null || m.rtf != null || m.params != null || m.ram != null || (m.lang_codes && m.lang_codes.length > 0)" class="inline-flex items-center gap-1 flex-wrap">
                      <Badge
                        v-if="m.wer != null"
                        variant="secondary"
                        :class="['text-[9px] px-1 py-0 border-transparent font-medium', werBadge(m.wer).cls]"
                      >{{ werBadge(m.wer).label }} <span class="opacity-50 font-normal">{{ +m.wer.toFixed(1) }}%</span></Badge>
                      <Badge
                        v-if="m.rtf != null"
                        variant="secondary"
                        :class="['text-[9px] px-1 py-0 border-transparent font-medium', rtfBadge(m.rtf).cls]"
                      >{{ rtfBadge(m.rtf).label }} <span class="opacity-50 font-normal">{{ +m.rtf.toFixed(2) }}x</span></Badge>
                      <Badge
                        v-if="m.params != null"
                        variant="secondary"
                        class="text-[9px] px-1 py-0 bg-slate-500/10 text-slate-600 border-transparent font-medium"
                      >{{ formatParams(m.params) }}</Badge>
                      <Badge
                        v-if="m.ram != null"
                        variant="secondary"
                        class="text-[9px] px-1 py-0 bg-cyan-500/10 text-cyan-600 border-transparent font-medium"
                      >RAM <span class="opacity-50 font-normal">~{{ formatRam(m.ram) }}</span></Badge>
                      <Badge
                        v-if="m.lang_codes && m.lang_codes.length > 0"
                        variant="secondary"
                        class="text-[9px] px-1 py-0 bg-indigo-500/10 text-indigo-600 border-transparent font-medium"
                      >{{ formatLangs(m.lang_codes) }}</Badge>
                    </span>
                  </div>
                </SelectItem>
              </SelectContent>
            </Select>
            <p v-else class="text-sm text-muted-foreground">
              {{ t('settings.transcription.noModels') }}
            </p>
          </div>

          <!-- Cloud ASR sub-settings (model name) -->
          <template v-if="engines.isCloudAsr && asrSelectedProvider">
            <div class="space-y-1">
              <Label class="text-sm font-medium">{{ t('settings.cloudAsr.model') }}</Label>
              <div v-if="asrModelOptions.length > 0" class="flex items-center gap-2">
                <Select
                  class="flex-1"
                  :model-value="asrModelSelectValue"
                  @update:model-value="onAsrModelSelect"
                >
                  <SelectTrigger class="w-full h-9 text-sm">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem
                      v-for="m in asrModelOptions"
                      :key="m"
                      :value="m"
                    >
                      {{ m }}
                    </SelectItem>
                    <SelectItem :value="CUSTOM_MODEL_VALUE">{{ t('settings.cloudAsr.custom') }}</SelectItem>
                  </SelectContent>
                </Select>
                <TooltipProvider :delay-duration="300">
                  <Tooltip>
                    <TooltipTrigger as-child>
                      <Button
                        variant="outline"
                        size="icon"
                        class="h-9 w-9 shrink-0"
                        :disabled="refreshingAsr"
                        @click="refreshAsrModels"
                      >
                        <Loader2 v-if="refreshingAsr" class="w-4 h-4 animate-spin" />
                        <RefreshCw v-else class="w-4 h-4" />
                      </Button>
                    </TooltipTrigger>
                    <TooltipContent side="bottom" :side-offset="4">{{ t('settings.models.refresh') }}</TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </div>
              <Input
                v-if="isCustomAsrModel"
                :value="settings.asrCloudModel"
                @input="onAsrModelInput"
                :placeholder="t('settings.cloudAsr.customPlaceholder')"
                class="h-9 text-sm mt-1.5"
              />
            </div>
          </template>

          <!-- Language -->
          <div class="space-y-1">
            <Label class="text-sm font-medium">{{ t('settings.transcription.language') }}</Label>
            <Select :model-value="settings.selectedLanguage" @update:model-value="onLanguageChange">
              <SelectTrigger class="w-full h-9 text-sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="lang in engines.languages"
                  :key="lang.code"
                  :value="lang.code"
                >
                  {{ lang.label }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          <!-- GPU Acceleration (local only) -->
          <div v-if="!engines.isCloudAsr" class="space-y-1">
            <Label class="text-sm font-medium">{{ t('settings.transcription.gpuMode') }}</Label>
            <Select :model-value="settings.gpuMode" @update:model-value="onGpuModeChange">
              <SelectTrigger class="w-full h-9 text-sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="auto">{{ t('settings.transcription.gpuMode.auto') }}</SelectItem>
                <SelectItem value="gpu">{{ t('settings.transcription.gpuMode.gpu') }}</SelectItem>
                <SelectItem value="cpu">{{ t('settings.transcription.gpuMode.cpu') }}</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>
      </div>

      <!-- Post-processing -->
      <div v-if="activeSection === 'postprocessing'">
        <h2 class="text-lg font-semibold mb-4">{{ t('settings.section.postProcessing') }}</h2>

        <div class="space-y-4">
          <div class="flex items-center justify-between gap-4">
            <Label class="text-sm shrink-0">{{ t('settings.postProcessing.vad') }}</Label>
            <Switch
              :model-value="settings.vadEnabled"
              @update:model-value="(v: boolean) => settings.setSetting('vad_enabled', String(v))"
            />
          </div>

          <div class="flex items-center justify-between gap-4">
            <Label class="text-sm shrink-0">{{ t('settings.postProcessing.hallucinations') }}</Label>
            <Switch
              :model-value="settings.hallucinationFilterEnabled"
              @update:model-value="onHallucinationFilterChange"
            />
          </div>

          <!-- Text cleanup toggle -->
          <div class="flex items-center justify-between gap-4">
            <Label class="text-sm shrink-0">{{ t('settings.postProcessing.textCleanup') }}</Label>
            <Switch
              :model-value="settings.textCleanupEnabled"
              @update:model-value="onTextCleanupChange"
            />
          </div>

          <!-- Cleanup model selector + sub-settings (only when cleanup enabled) -->
          <div
            v-if="settings.textCleanupEnabled"
            class="space-y-4 pl-4 border-l-2 border-border"
          >
            <!-- Model selector -->
            <div class="space-y-1">
              <Label class="text-xs text-muted-foreground">{{ t('settings.postProcessing.cleanupModel') }}</Label>
              <Select
                v-if="engines.cleanupModels.length > 0"
                :model-value="settings.cleanupModelId"
                @update:model-value="onCleanupModelChange"
              >
                <SelectTrigger class="w-full h-9 text-sm">
                  <span v-if="selectedCleanupModel" class="inline-flex items-center gap-1.5 truncate">
                    <span class="truncate">{{ selectedCleanupModel.label }}</span>
                    <Badge
                      variant="secondary"
                      :class="['text-[9px] px-1 py-0 border-transparent font-medium shrink-0', cleanupGroupClass(selectedCleanupModel.group)]"
                    >{{ cleanupGroupLabel(selectedCleanupModel.group) }}</Badge>
                  </span>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem v-for="m in engines.cleanupModels" :key="m.id" :value="m.id">
                    <div class="flex flex-col gap-0.5">
                      <span class="flex items-center gap-1.5">
                        {{ m.label }}
                        <Badge
                          v-if="m.recommended"
                          variant="secondary"
                          class="text-[9px] px-1 py-0 bg-emerald-500/10 text-emerald-600 border-transparent font-medium"
                        >{{ t('settings.cleanup.recommended') }}</Badge>
                        <Badge
                          variant="secondary"
                          :class="['text-[9px] px-1 py-0 border-transparent font-medium', cleanupGroupClass(m.group)]"
                        >{{ cleanupGroupLabel(m.group) }}</Badge>
                      </span>
                      <span v-if="m.params != null || m.ram != null || (m.lang_codes && m.lang_codes.length > 0)" class="inline-flex items-center gap-1 flex-wrap">
                        <Badge
                          v-if="m.params != null"
                          variant="secondary"
                          class="text-[9px] px-1 py-0 bg-slate-500/10 text-slate-600 border-transparent font-medium"
                        >{{ formatParams(m.params) }}</Badge>
                        <Badge
                          v-if="m.ram != null"
                          variant="secondary"
                          class="text-[9px] px-1 py-0 bg-cyan-500/10 text-cyan-600 border-transparent font-medium"
                        >RAM <span class="opacity-50 font-normal">~{{ formatRam(m.ram) }}</span></Badge>
                        <Badge
                          v-if="m.lang_codes && m.lang_codes.length > 0"
                          variant="secondary"
                          class="text-[9px] px-1 py-0 bg-indigo-500/10 text-indigo-600 border-transparent font-medium"
                        >{{ formatLangs(m.lang_codes) }}</Badge>
                      </span>
                    </div>
                  </SelectItem>
                </SelectContent>
              </Select>
              <p v-else class="text-sm text-muted-foreground">
                {{ t('settings.postProcessing.cleanupModel.none') }}
              </p>
            </div>

            <!-- Cloud LLM sub-settings (provider already selected via model dropdown) -->
            <template v-if="engines.isCloudLlm && llmSelectedProvider">
              <div class="space-y-1">
                <Label class="text-xs text-muted-foreground">{{ t('settings.llm.model') }}</Label>
                <div class="flex items-center gap-2">
                  <Select
                    v-if="!isCustomLlmModel"
                    class="flex-1"
                    :model-value="settings.llmModel"
                    @update:model-value="onLlmModelSelect"
                  >
                    <SelectTrigger class="w-full h-9 text-sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem
                        v-for="m in llmModelOptions"
                        :key="m"
                        :value="m"
                      >
                        {{ m }}
                      </SelectItem>
                    </SelectContent>
                  </Select>
                  <Input
                    v-else
                    :value="settings.llmModel"
                    @input="onLlmModelInput"
                    class="h-9 text-sm flex-1"
                  />
                  <TooltipProvider :delay-duration="300">
                    <Tooltip>
                      <TooltipTrigger as-child>
                        <Button
                          variant="outline"
                          size="icon"
                          class="h-9 w-9 shrink-0"
                          :disabled="refreshingLlm"
                          @click="refreshLlmModels"
                        >
                          <Loader2 v-if="refreshingLlm" class="w-4 h-4 animate-spin" />
                          <RefreshCw v-else class="w-4 h-4" />
                        </Button>
                      </TooltipTrigger>
                      <TooltipContent side="bottom" :side-offset="4">{{ t('settings.models.refresh') }}</TooltipContent>
                    </Tooltip>
                  </TooltipProvider>
                </div>
              </div>

              <!-- Token hard cap (cloud) -->
              <div class="space-y-2">
                <div class="flex items-center justify-between">
                  <Label class="text-xs text-muted-foreground">{{ t('settings.llm.maxTokens') }}</Label>
                  <span class="text-xs text-muted-foreground tabular-nums">{{ settings.llmMaxTokens }}</span>
                </div>
                <Slider
                  :model-value="[settings.llmMaxTokens]"
                  :min="128"
                  :max="8192"
                  :step="128"
                  @update:model-value="onMaxTokensSliderUpdate"
                  @value-commit="onMaxTokensSliderCommit"
                />
              </div>
            </template>

            <!-- Local LLM sub-settings (token hard cap) -->
            <template v-if="engines.isLocalLlm">
              <div class="space-y-2">
                <div class="flex items-center justify-between">
                  <Label class="text-xs text-muted-foreground">{{ t('settings.llm.maxTokens') }}</Label>
                  <span class="text-xs text-muted-foreground tabular-nums">{{ settings.llmMaxTokens }}</span>
                </div>
                <Slider
                  :model-value="[settings.llmMaxTokens]"
                  :min="128"
                  :max="8192"
                  :step="128"
                  @update:model-value="onMaxTokensSliderUpdate"
                  @value-commit="onMaxTokensSliderCommit"
                />
              </div>
            </template>
          </div>
        </div>
      </div>

      <!-- Shortcuts -->
      <div v-if="activeSection === 'shortcuts'">
        <h2 class="text-lg font-semibold mb-4">{{ t('settings.section.shortcuts') }}</h2>

        <div class="space-y-4">
          <!-- Recording mode toggle -->
          <div class="flex items-center justify-between">
            <Label class="text-sm font-medium">{{ t('settings.shortcut.mode') }}</Label>
            <SegmentedToggle
              :model-value="settings.recordingMode"
              :options="[
                { value: 'push_to_talk', label: t('settings.shortcut.mode.pushToTalk') },
                { value: 'toggle', label: t('settings.shortcut.mode.toggle') },
              ]"
              @update:model-value="onRecordingModeChange"
            />
          </div>

          <div class="space-y-2">
            <Label class="text-sm font-medium">{{ t('settings.shortcut.record') }}</Label>
            <ShortcutCapture
              :model-value="settings.hotkey"
              @update:model-value="onHotkeyChange"
            />
          </div>

          <div class="space-y-2">
            <Label class="text-sm font-medium">{{ t('settings.shortcut.cancel') }}</Label>
            <div class="flex gap-2">
              <ShortcutCapture
                class="flex-1"
                :model-value="settings.cancelShortcut"
                @update:model-value="onCancelShortcutChange"
              />
              <Button
                variant="outline"
                size="sm"
                class="shrink-0 h-9"
                @click="onDisableCancel"
              >
                {{ t('settings.shortcut.cancel.none') }}
              </Button>
            </div>
          </div>
        </div>
      </div>

      <!-- Microphone -->
      <div v-if="activeSection === 'microphone'">
        <h2 class="text-lg font-semibold mb-4">{{ t('settings.section.microphone') }}</h2>

        <div class="space-y-4">
          <div class="space-y-2">
            <Label class="text-sm font-medium">{{ t('settings.microphone') }}</Label>
            <div class="flex items-center gap-2">
              <Select
                :model-value="selectedDeviceUid"
                :disabled="engines.audioDevices.length === 0"
                @update:model-value="onDeviceChange"
                class="flex-1"
              >
                <SelectTrigger class="w-full h-9 text-sm">
                  <span v-if="selectedDevice" class="inline-flex items-center gap-1.5 truncate">
                    <component :is="deviceIcon(selectedDevice.transport_type)" class="w-3.5 h-3.5 shrink-0 text-muted-foreground" />
                    <span class="truncate">{{ selectedDevice.name }}{{ selectedDevice.is_default ? ` (${t('settings.microphone.defaultTag')})` : '' }}</span>
                  </span>
                  <span v-else class="text-muted-foreground">{{ t('menu.noDevices') }}</span>
                </SelectTrigger>
                <SelectContent>
                  <SelectItem
                    v-for="device in engines.audioDevices"
                    :key="device.uid"
                    :value="device.uid"
                  >
                    <span class="inline-flex items-center gap-1.5">
                      <component :is="deviceIcon(device.transport_type)" class="w-3.5 h-3.5 shrink-0 text-muted-foreground" />
                      <span>{{ device.name }}{{ device.is_default ? ` (${t('settings.microphone.defaultTag')})` : '' }}</span>
                    </span>
                  </SelectItem>
                </SelectContent>
              </Select>
              <Button
                variant="outline"
                size="sm"
                class="shrink-0 h-9 w-20"
                :disabled="engines.audioDevices.length === 0"
                @click="isTesting ? stopMicTest() : startMicTest()"
              >
                {{ isTesting ? t('settings.microphone.stop') : t('settings.microphone.test') }}
              </Button>
            </div>
            <div v-if="isTesting" class="rounded-md border border-border bg-muted/30 px-3 py-2">
              <SpectrumBars :spectrum="testSpectrum" size="md" />
            </div>
          </div>

          <div class="flex items-center justify-between gap-4">
            <Label class="text-sm shrink-0">{{ t('settings.microphone.ducking') }}</Label>
            <Switch
              :model-value="settings.audioDuckingEnabled"
              @update:model-value="onAudioDuckingChange"
            />
          </div>

          <div
            v-if="settings.audioDuckingEnabled"
            class="space-y-2 pl-4 border-l-2 border-border"
          >
            <div class="flex items-center justify-between">
              <Label class="text-xs text-muted-foreground">{{ t('settings.microphone.duckingLevel') }}</Label>
              <span class="text-xs text-muted-foreground tabular-nums">
                {{ duckingSliderValue >= 100 ? t('settings.microphone.duckingMute') : `${Math.round(duckingSliderValue)}%` }}
              </span>
            </div>
            <Slider
              :model-value="[duckingSliderValue]"
              :min="5"
              :max="100"
              :step="5"
              @update:model-value="onDuckingSliderUpdate"
              @value-commit="onDuckingSliderCommit"
            />
          </div>
        </div>
      </div>
    </div>

    <!-- Remove provider confirmation dialog -->
    <ConfirmDialog
      v-model:open="showRemoveConfirm"
      :title="t('settings.providers.removeConfirm')"
      :description="t('settings.providers.removeConfirmDesc')"
      :confirm-label="t('modelManager.delete')"
      @confirm="confirmRemoveProvider"
    />
  </div>
</template>
