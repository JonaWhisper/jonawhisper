<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '@/stores/app'
import type { LlmConfig } from '@/stores/app'
import { Settings, Sparkles, Keyboard, Mic, Laptop, Usb, Bluetooth, Waves, HardDrive, Zap, Monitor } from 'lucide-vue-next'
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
import SpectrumBars from '@/components/SpectrumBars.vue'
import ShortcutCapture from '@/components/ShortcutCapture.vue'
import { serializeShortcut } from '@/utils/shortcut'

const { t } = useI18n()
const store = useAppStore()

// Active section
const activeSection = ref('general')

const sections = [
  { id: 'general', icon: Settings, label: 'settings.section.general' },
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

// LLM config — local refs for form fields, synced with store on mount
const llmProvider = ref('openai')
const llmApiUrl = ref('')
const llmApiKey = ref('')
const llmModel = ref('')
const llmSaved = ref(false)

const llmApiUrlPlaceholder = computed(() =>
  llmProvider.value === 'anthropic'
    ? t('settings.llm.apiUrl.placeholder.anthropic')
    : t('settings.llm.apiUrl.placeholder.openai')
)

const llmModelPlaceholder = computed(() =>
  llmProvider.value === 'anthropic'
    ? t('settings.llm.model.placeholder.anthropic')
    : t('settings.llm.model.placeholder.openai')
)

function loadLlmFormFields() {
  const c = store.llmConfig
  llmProvider.value = c.provider || 'openai'
  llmApiUrl.value = c.api_url || ''
  llmApiKey.value = c.api_key || ''
  llmModel.value = c.model || ''
}

async function onLlmEnabledChange(enabled: boolean) {
  const config: LlmConfig = {
    enabled,
    provider: llmProvider.value,
    api_url: llmApiUrl.value,
    api_key: llmApiKey.value,
    model: llmModel.value,
  }
  await store.setLlmConfig(config)
}

function onLlmProviderChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  llmProvider.value = value
}

async function saveLlmConfig() {
  const config: LlmConfig = {
    enabled: store.llmConfig.enabled,
    provider: llmProvider.value,
    api_url: llmApiUrl.value,
    api_key: llmApiKey.value,
    model: llmModel.value,
  }
  await store.setLlmConfig(config)
  llmSaved.value = true
  setTimeout(() => { llmSaved.value = false }, 1500)
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
  ])
  loadLlmFormFields()

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
    <div class="w-40 min-w-[8rem] border-r border-border bg-muted/30 overflow-y-auto flex-shrink-0">
      <div class="p-3">
        <h2 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
          {{ t('settings.title') }}
        </h2>
      </div>
      <div class="space-y-0.5 px-1">
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
          <div class="space-y-1.5">
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
            class="space-y-3 pl-4 border-l-2 border-border"
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
                :model-value="store.llmConfig.enabled"
                @update:model-value="onLlmEnabledChange"
              />
            </div>

            <!-- LLM config form -->
            <div
              v-if="store.llmConfig.enabled"
              class="space-y-3 pl-4 border-l-2 border-border"
            >
              <div class="space-y-1">
                <Label class="text-xs text-muted-foreground">{{ t('settings.llm.provider') }}</Label>
                <Select :model-value="llmProvider" @update:model-value="onLlmProviderChange">
                  <SelectTrigger class="w-full h-8 text-sm">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="openai">{{ t('settings.llm.provider.openai') }}</SelectItem>
                    <SelectItem value="anthropic">{{ t('settings.llm.provider.anthropic') }}</SelectItem>
                  </SelectContent>
                </Select>
              </div>

              <div class="space-y-1">
                <Label class="text-xs text-muted-foreground">{{ t('settings.llm.apiUrl') }}</Label>
                <Input
                  v-model="llmApiUrl"
                  :placeholder="llmApiUrlPlaceholder"
                  class="h-8 text-sm"
                />
              </div>

              <div class="space-y-1">
                <Label class="text-xs text-muted-foreground">{{ t('settings.llm.apiKey') }}</Label>
                <Input
                  v-model="llmApiKey"
                  type="password"
                  :placeholder="t('settings.llm.apiKey.placeholder')"
                  class="h-8 text-sm"
                />
              </div>

              <div class="space-y-1">
                <Label class="text-xs text-muted-foreground">{{ t('settings.llm.model') }}</Label>
                <Input
                  v-model="llmModel"
                  :placeholder="llmModelPlaceholder"
                  class="h-8 text-sm"
                />
              </div>

              <Button
                size="sm"
                class="w-full"
                @click="saveLlmConfig"
              >
                {{ llmSaved ? t('settings.llm.saved') : t('settings.llm.save') }}
              </Button>
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
                  ? 'bg-accent text-accent-foreground font-medium'
                  : 'hover:bg-accent/50 text-muted-foreground'"
                @click="onRecordingModeChange('push_to_talk')"
              >
                {{ t('settings.shortcut.mode.pushToTalk') }}
              </button>
              <button
                class="px-3 py-1.5 text-sm border-l border-border transition-colors whitespace-nowrap"
                :class="store.recordingMode === 'toggle'
                  ? 'bg-accent text-accent-foreground font-medium'
                  : 'hover:bg-accent/50 text-muted-foreground'"
                @click="onRecordingModeChange('toggle')"
              >
                {{ t('settings.shortcut.mode.toggle') }}
              </button>
            </div>
          </div>

          <div class="space-y-1.5">
            <Label class="text-sm font-medium">{{ t('settings.shortcut.record') }}</Label>
            <ShortcutCapture
              :model-value="store.hotkey"
              @update:model-value="onHotkeyChange"
            />
          </div>

          <div class="space-y-1.5">
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
          <div class="space-y-1.5">
            <Label class="text-sm font-medium">{{ t('settings.microphone') }}</Label>
            <Select
              :model-value="selectedDeviceUid"
              @update:model-value="onDeviceChange"
            >
              <SelectTrigger class="w-full">
                <span v-if="selectedDevice" class="inline-flex items-center gap-1.5 truncate">
                  <component :is="deviceIcon(selectedDevice.transport_type)" class="w-3.5 h-3.5 shrink-0 text-muted-foreground" />
                  <span class="truncate">{{ selectedDevice.name }}{{ selectedDevice.is_default ? ` (${t('settings.microphone.defaultTag')})` : '' }}</span>
                </span>
                <SelectValue v-else />
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
  </div>
</template>
