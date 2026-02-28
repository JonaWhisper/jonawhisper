<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore, type Provider } from '@/stores/app'
import { Settings, Cloud, Sparkles, Keyboard, Mic, AudioLines, Laptop, Usb, Bluetooth, Waves, HardDrive, Zap, Monitor, Pencil, Trash2, Plus } from 'lucide-vue-next'
import type { Component } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { i18n } from '@/main'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import SpectrumBars from '@/components/SpectrumBars.vue'
import ShortcutCapture from '@/components/ShortcutCapture.vue'
import ProviderForm from '@/components/ProviderForm.vue'
import { serializeShortcut } from '@/utils/shortcut'

const { t } = useI18n()
const store = useAppStore()

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
  await store.setSetting('app_locale', value)
  // Rust handles tray labels; resolve effective locale for frontend
  const locale = await invoke<string>('get_system_locale')
  i18n.global.locale.value = locale as 'fr' | 'en'
}

async function onPostProcessingChange(enabled: boolean) {
  await store.setSetting('post_processing_enabled', String(enabled))
}

async function onHallucinationFilterChange(enabled: boolean) {
  await store.setSetting('hallucination_filter_enabled', String(enabled))
}

async function onHotkeyChange(value: string) {
  await store.setSetting('hotkey', value)
}

async function onCancelShortcutChange(value: string) {
  await store.setSetting('cancel_shortcut', value)
}

function onDisableCancel() {
  const disabled = serializeShortcut({ key_code: 0, modifiers: 0, kind: 'Key' })
  onCancelShortcutChange(disabled)
}

async function onRecordingModeChange(mode: string) {
  await store.setSetting('recording_mode', mode)
}

const TRANSPORT_ICONS: Record<string, Component> = {
  BuiltIn: Laptop, USB: Usb, Bluetooth: Bluetooth,
  Virtual: Waves, Aggregate: HardDrive, Thunderbolt: Zap,
  HDMI: Monitor, Unknown: Mic,
}
function deviceIcon(type: string): Component { return TRANSPORT_ICONS[type] ?? Mic }

// Selected device UID: use the stored preference, or the default device UID
const selectedDeviceUid = computed(() => {
  const settings = store.audioDevices
  const stored = settings.find(d => d.uid === store.selectedInputDeviceUid)
  if (stored) return stored.uid
  const def = settings.find(d => d.is_default)
  return def?.uid ?? ''
})

const selectedDevice = computed(() =>
  store.audioDevices.find(d => d.uid === selectedDeviceUid.value)
)

async function onDeviceChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  // If selecting the default device, store empty string (= use system default)
  const defaultDevice = store.audioDevices.find(d => d.is_default)
  const uid = (defaultDevice && value === defaultDevice.uid) ? '' : value
  await store.setSetting('selected_input_device_uid', uid)
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
  await store.addProvider(provider)
  showAddForm.value = false
}

function startEditProvider(provider: Provider) {
  editingProviderIds.value.add(provider.id)
}

function cancelEditProvider(providerId: string) {
  editingProviderIds.value.delete(providerId)
}

async function saveEditedProvider(provider: Provider) {
  await store.updateProvider(provider)
  editingProviderIds.value.delete(provider.id)
}

function requestRemoveProvider(provider: Provider) {
  removeTarget.value = provider
  showRemoveConfirm.value = true
}

async function confirmRemoveProvider() {
  if (removeTarget.value) {
    await store.removeProvider(removeTarget.value.id)
  }
  showRemoveConfirm.value = false
  removeTarget.value = null
}

// -- Transcription (ASR source + model + language) --
const OPENAI_ASR_MODELS = ['whisper-1', 'gpt-4o-transcribe', 'gpt-4o-mini-transcribe']
const CUSTOM_MODEL_VALUE = '_custom'

// Local model: only show downloaded models
const localDownloadedModels = computed(() =>
  store.models.filter(m => {
    if (m.download_type.type === 'System') return true
    return m.is_downloaded
  })
)

async function onLocalModelChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await store.setSetting('selected_model_id', value)
}

async function onLanguageChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await store.setSetting('selected_language', value)
}

const asrCapableProviders = computed(() =>
  store.providers.filter(p => p.kind === 'OpenAI' || p.kind === 'Custom')
)

const asrSelectedProvider = computed(() =>
  store.providers.find(p => p.id === store.asrProviderId)
)

const asrModelOptions = computed(() => {
  const provider = asrSelectedProvider.value
  if (!provider) return []
  if (provider.kind === 'OpenAI') return OPENAI_ASR_MODELS
  return [] // Custom → free text only
})

const isCustomAsrModel = computed(() => {
  if (asrModelOptions.value.length === 0) return true
  return !asrModelOptions.value.includes(store.asrCloudModel)
})

// For the dropdown: show the actual value if it's in the list, otherwise show "_custom"
const asrModelSelectValue = computed(() => {
  if (asrModelOptions.value.length === 0) return CUSTOM_MODEL_VALUE
  if (asrModelOptions.value.includes(store.asrCloudModel)) return store.asrCloudModel
  return CUSTOM_MODEL_VALUE
})

async function onAsrProviderChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await store.setSetting('asr_provider_id', value === '_local' ? '' : value)
  // Reset to default model when changing provider
  if (value !== '_local') {
    const provider = store.providers.find(p => p.id === value)
    const defaultModel = provider?.kind === 'OpenAI' ? 'whisper-1' : ''
    await store.setSetting('asr_cloud_model', defaultModel)
  }
}

async function onAsrModelSelect(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  if (value === CUSTOM_MODEL_VALUE) {
    // Switch to custom mode — clear the model so the user types a new one
    await store.setSetting('asr_cloud_model', '')
    return
  }
  await store.setSetting('asr_cloud_model', value)
}

let asrModelDebounce: ReturnType<typeof setTimeout> | null = null

function onAsrModelInput(event: Event) {
  const value = (event.target as HTMLInputElement).value
  store.asrCloudModel = value
  if (asrModelDebounce) clearTimeout(asrModelDebounce)
  asrModelDebounce = setTimeout(() => {
    store.setSetting('asr_cloud_model', value)
  }, 500)
}

// -- LLM config --
const OPENAI_MODELS = ['gpt-4o-mini', 'gpt-4o']
const ANTHROPIC_MODELS = ['claude-haiku-4-5-20251001', 'claude-sonnet-4-5-20250514', 'claude-opus-4-6-20250626']

const llmSelectedProvider = computed(() =>
  store.providers.find(p => p.id === store.llmProviderId)
)

const llmModelOptions = computed(() => {
  const provider = llmSelectedProvider.value
  if (!provider) return []
  switch (provider.kind) {
    case 'OpenAI': return OPENAI_MODELS
    case 'Anthropic': return ANTHROPIC_MODELS
    default: return []
  }
})

const isCustomLlmModel = computed(() => llmModelOptions.value.length === 0)

let llmModelDebounce: ReturnType<typeof setTimeout> | null = null

async function onLlmEnabledChange(enabled: boolean) {
  await store.setSetting('llm_enabled', String(enabled))
}

async function onLlmProviderChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await store.setSetting('llm_provider_id', value)
  // Reset model when changing provider
  await store.setSetting('llm_model', '')
}

async function onLlmModelSelect(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await store.setSetting('llm_model', value)
}

function onLlmModelInput(event: Event) {
  const value = (event.target as HTMLInputElement).value
  store.llmModel = value
  if (llmModelDebounce) clearTimeout(llmModelDebounce)
  llmModelDebounce = setTimeout(() => {
    store.setSetting('llm_model', value)
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
    store.fetchSettings(),
    store.fetchAudioDevices(),
    store.fetchProviders(),
    store.fetchEngines(),
    store.fetchModels(),
    store.fetchLanguages(),
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
            <Select :model-value="store.appLocale" @update:model-value="onLocaleChange">
              <SelectTrigger class="w-full">
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
          <div v-if="store.providers.length === 0 && !showAddForm" class="text-sm text-muted-foreground">
            {{ t('settings.providers.empty') }}
          </div>

          <div v-for="provider in store.providers" :key="provider.id" class="rounded-md border border-border">
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
          <!-- Source: local or cloud provider -->
          <div class="space-y-1">
            <Label class="text-sm font-medium">{{ t('settings.transcription.source') }}</Label>
            <Select :model-value="store.asrProviderId || '_local'" @update:model-value="onAsrProviderChange">
              <SelectTrigger class="w-full h-9 text-sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="_local">{{ t('settings.transcription.local') }}</SelectItem>
                <SelectItem
                  v-for="p in asrCapableProviders"
                  :key="p.id"
                  :value="p.id"
                >
                  {{ p.name }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          <!-- Cloud model selection -->
          <div v-if="store.asrProviderId" class="space-y-1">
            <Label class="text-sm font-medium">{{ t('settings.cloudAsr.model') }}</Label>
            <!-- Known models: dropdown with Custom option -->
            <Select
              v-if="asrModelOptions.length > 0"
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
            <!-- Custom model text input (shown for Custom providers, or when "Custom" is selected) -->
            <Input
              v-if="isCustomAsrModel"
              :value="store.asrCloudModel"
              @input="onAsrModelInput"
              :placeholder="t('settings.cloudAsr.customPlaceholder')"
              class="h-9 text-sm mt-1.5"
            />
          </div>

          <!-- Local model selection -->
          <div v-if="!store.asrProviderId" class="space-y-1">
            <Label class="text-sm font-medium">{{ t('settings.transcription.model') }}</Label>
            <Select
              v-if="localDownloadedModels.length > 0"
              :model-value="store.selectedModelId"
              @update:model-value="onLocalModelChange"
            >
              <SelectTrigger class="w-full h-9 text-sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="m in localDownloadedModels"
                  :key="m.id"
                  :value="m.id"
                >
                  {{ m.label }}
                </SelectItem>
              </SelectContent>
            </Select>
            <p v-else class="text-sm text-muted-foreground">
              {{ t('settings.transcription.noModels') }}
            </p>
          </div>

          <!-- Language -->
          <div class="space-y-1">
            <Label class="text-sm font-medium">{{ t('settings.transcription.language') }}</Label>
            <Select :model-value="store.selectedLanguage" @update:model-value="onLanguageChange">
              <SelectTrigger class="w-full h-9 text-sm">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="lang in store.languages"
                  :key="lang.code"
                  :value="lang.code"
                >
                  {{ lang.label }}
                </SelectItem>
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
            <Label class="text-sm shrink-0">{{ t('settings.postProcessing.enable') }}</Label>
            <Switch
              :model-value="store.postProcessingEnabled"
              @update:model-value="onPostProcessingChange"
            />
          </div>

          <div
            class="space-y-4 pl-4 border-l-2 border-border"
            :class="{ 'opacity-40 pointer-events-none': !store.postProcessingEnabled }"
          >
            <div class="flex items-center justify-between gap-4">
              <Label class="text-sm shrink-0">{{ t('settings.postProcessing.hallucinations') }}</Label>
              <Switch
                :model-value="store.hallucinationFilterEnabled"
                @update:model-value="onHallucinationFilterChange"
              />
            </div>
            <div class="flex items-center justify-between gap-4">
              <Label class="text-sm shrink-0">{{ t('settings.postProcessing.llm') }}</Label>
              <Switch
                :model-value="store.llmEnabled"
                @update:model-value="onLlmEnabledChange"
              />
            </div>

            <!-- LLM config -->
            <div
              v-if="store.llmEnabled"
              class="space-y-4 pl-4 border-l-2 border-border"
            >
              <div v-if="store.providers.length === 0" class="text-sm text-muted-foreground">
                {{ t('settings.llm.noProviders') }}
              </div>

              <template v-else>
                <div class="space-y-1">
                  <Label class="text-xs text-muted-foreground">{{ t('settings.llm.provider') }}</Label>
                  <Select :model-value="store.llmProviderId" @update:model-value="onLlmProviderChange">
                    <SelectTrigger class="w-full h-9 text-sm">
                      <SelectValue />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem
                        v-for="p in store.providers"
                        :key="p.id"
                        :value="p.id"
                      >
                        {{ p.name }}
                      </SelectItem>
                    </SelectContent>
                  </Select>
                </div>

                <div v-if="llmSelectedProvider" class="space-y-1">
                  <Label class="text-xs text-muted-foreground">{{ t('settings.llm.model') }}</Label>
                  <!-- Preconfigured: dropdown -->
                  <Select
                    v-if="!isCustomLlmModel"
                    :model-value="store.llmModel"
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
                  <!-- Custom: free text input -->
                  <Input
                    v-else
                    :value="store.llmModel"
                    @input="onLlmModelInput"
                    class="h-9 text-sm"
                  />
                </div>
              </template>
            </div>
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
            <div class="inline-flex rounded-md border border-border overflow-hidden">
              <button
                class="px-3 py-1.5 text-sm transition-colors whitespace-nowrap"
                :class="store.recordingMode === 'push_to_talk'
                  ? 'bg-accent text-accent-foreground'
                  : 'hover:bg-accent/50 text-muted-foreground'"
                @click="onRecordingModeChange('push_to_talk')"
              >
                {{ t('settings.shortcut.mode.pushToTalk') }}
              </button>
              <button
                class="px-3 py-1.5 text-sm border-l border-border transition-colors whitespace-nowrap"
                :class="store.recordingMode === 'toggle'
                  ? 'bg-accent text-accent-foreground'
                  : 'hover:bg-accent/50 text-muted-foreground'"
                @click="onRecordingModeChange('toggle')"
              >
                {{ t('settings.shortcut.mode.toggle') }}
              </button>
            </div>
          </div>

          <div class="space-y-2">
            <Label class="text-sm font-medium">{{ t('settings.shortcut.record') }}</Label>
            <ShortcutCapture
              :model-value="store.hotkey"
              @update:model-value="onHotkeyChange"
            />
          </div>

          <div class="space-y-2">
            <Label class="text-sm font-medium">{{ t('settings.shortcut.cancel') }}</Label>
            <div class="flex gap-2">
              <ShortcutCapture
                class="flex-1"
                :model-value="store.cancelShortcut"
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
            <Select
              :model-value="selectedDeviceUid"
              :disabled="store.audioDevices.length === 0"
              @update:model-value="onDeviceChange"
            >
              <SelectTrigger class="w-full">
                <span v-if="selectedDevice" class="inline-flex items-center gap-1.5 truncate">
                  <component :is="deviceIcon(selectedDevice.transport_type)" class="w-3.5 h-3.5 shrink-0 text-muted-foreground" />
                  <span class="truncate">{{ selectedDevice.name }}{{ selectedDevice.is_default ? ` (${t('settings.microphone.defaultTag')})` : '' }}</span>
                </span>
                <span v-else class="text-muted-foreground">{{ t('menu.noDevices') }}</span>
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="device in store.audioDevices"
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
          </div>

          <div class="space-y-3">
            <Button
              variant="outline"
              size="sm"
              class="w-20"
              :disabled="store.audioDevices.length === 0"
              @click="isTesting ? stopMicTest() : startMicTest()"
            >
              {{ isTesting ? t('settings.microphone.stop') : t('settings.microphone.test') }}
            </Button>
            <div v-if="isTesting" class="rounded-md border border-border bg-muted/30 px-3 py-2">
              <SpectrumBars :spectrum="testSpectrum" size="md" />
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Remove provider confirmation dialog -->
    <AlertDialog :open="showRemoveConfirm" @update:open="showRemoveConfirm = $event">
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{{ t('settings.providers.removeConfirm') }}</AlertDialogTitle>
          <AlertDialogDescription>
            {{ t('settings.providers.removeConfirmDesc') }}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel @click="showRemoveConfirm = false">{{ t('modelManager.cancel') }}</AlertDialogCancel>
          <AlertDialogAction @click="confirmRemoveProvider" class="bg-destructive text-destructive-foreground hover:bg-destructive/90">
            {{ t('modelManager.delete') }}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  </div>
</template>
