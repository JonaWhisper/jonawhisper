<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { listen } from '@tauri-apps/api/event'
import { i18n } from '@/main'
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card'
import { Switch } from '@/components/ui/switch'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Progress } from '@/components/ui/progress'

const { t } = useI18n()
const store = useAppStore()

// Mic test
const isTesting = ref(false)
const micLevel = ref(0)
let testTimeout: ReturnType<typeof setTimeout> | null = null
let spectrumUnlisten: (() => void) | null = null

const hotkeyOptions = [
  { value: 'right_command', label: 'hotkey.rightCommand' },
  { value: 'right_option', label: 'hotkey.rightOption' },
  { value: 'right_control', label: 'hotkey.rightControl' },
  { value: 'right_shift', label: 'hotkey.rightShift' },
]

const cancelShortcutOptions = [
  { value: 'escape', label: 'settings.shortcut.cancel.escape' },
  { value: 'none', label: 'settings.shortcut.cancel.none' },
]

const localeOptions = [
  { value: 'auto', label: 'settings.locale.auto' },
  { value: 'fr', label: 'settings.locale.fr' },
  { value: 'en', label: 'settings.locale.en' },
]

async function onLocaleChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await store.setSetting('app_locale', value)
  // Apply locale immediately
  if (value === 'auto') {
    i18n.global.locale.value = navigator.language.startsWith('fr') ? 'fr' : 'en'
  } else {
    i18n.global.locale.value = value as 'fr' | 'en'
  }
}

async function onPostProcessingChange(enabled: boolean) {
  await store.setSetting('post_processing_enabled', String(enabled))
}

async function onHallucinationFilterChange(enabled: boolean) {
  await store.setSetting('hallucination_filter_enabled', String(enabled))
}

async function onHotkeyChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await store.setHotkey(value)
}

async function onCancelShortcutChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await store.setSetting('cancel_shortcut', value)
}

async function onDeviceChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await store.setSetting('selected_input_device_uid', value === '__default__' ? '' : value)
}

async function startMicTest() {
  if (isTesting.value) return
  isTesting.value = true
  micLevel.value = 0

  // Listen to spectrum data for level metering
  spectrumUnlisten = await listen<number[]>('spectrum-data', (event) => {
    if (!isTesting.value) return
    // Compute average level from spectrum bands
    const bands = event.payload
    const avg = bands.reduce((a, b) => a + b, 0) / bands.length
    micLevel.value = Math.min(100, Math.round(avg * 100))
  })

  testTimeout = setTimeout(() => {
    stopMicTest()
  }, 3000)
}

function stopMicTest() {
  isTesting.value = false
  micLevel.value = 0
  if (spectrumUnlisten) {
    spectrumUnlisten()
    spectrumUnlisten = null
  }
  if (testTimeout) {
    clearTimeout(testTimeout)
    testTimeout = null
  }
}

onMounted(async () => {
  await Promise.all([
    store.fetchSettings(),
    store.fetchAudioDevices(),
  ])
})

onUnmounted(() => {
  stopMicTest()
})
</script>

<template>
  <div class="flex flex-col h-screen p-5 select-none overflow-y-auto">
    <h1 class="text-lg font-bold mb-4">{{ t('settings.title') }}</h1>

    <div class="space-y-3">
      <!-- Locale -->
      <Card>
        <CardHeader class="pb-2 pt-3 px-4">
          <CardTitle class="text-sm font-medium">{{ t('settings.locale') }}</CardTitle>
        </CardHeader>
        <CardContent class="px-4 pb-3">
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
        </CardContent>
      </Card>

      <!-- Post-processing -->
      <Card>
        <CardHeader class="pb-2 pt-3 px-4">
          <CardTitle class="text-sm font-medium">{{ t('settings.postProcessing') }}</CardTitle>
        </CardHeader>
        <CardContent class="px-4 pb-3 space-y-3">
          <div class="flex items-center justify-between">
            <Label class="text-sm">{{ t('settings.postProcessing.enable') }}</Label>
            <Switch
              :checked="store.postProcessingEnabled"
              @update:checked="onPostProcessingChange"
            />
          </div>
          <div
            class="flex items-center justify-between pl-4"
            :class="{ 'opacity-40 pointer-events-none': !store.postProcessingEnabled }"
          >
            <Label class="text-sm">{{ t('settings.postProcessing.hallucinations') }}</Label>
            <Switch
              :checked="store.hallucinationFilterEnabled"
              @update:checked="onHallucinationFilterChange"
            />
          </div>
          <div
            class="flex items-center justify-between pl-4"
            :class="{ 'opacity-40 pointer-events-none': !store.postProcessingEnabled }"
          >
            <Label class="text-sm text-muted-foreground">{{ t('settings.postProcessing.llm') }}</Label>
            <Button variant="outline" size="sm" disabled>
              {{ t('settings.postProcessing.llmConfigure') }}
            </Button>
          </div>
        </CardContent>
      </Card>

      <!-- Shortcuts -->
      <Card>
        <CardHeader class="pb-2 pt-3 px-4">
          <CardTitle class="text-sm font-medium">{{ t('settings.shortcut') }}</CardTitle>
        </CardHeader>
        <CardContent class="px-4 pb-3 space-y-3">
          <div class="space-y-1">
            <Label class="text-xs text-muted-foreground">{{ t('settings.shortcut.record') }}</Label>
            <Select :model-value="store.hotkey" @update:model-value="onHotkeyChange">
              <SelectTrigger class="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="opt in hotkeyOptions"
                  :key="opt.value"
                  :value="opt.value"
                >
                  {{ t(opt.label) }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div class="space-y-1">
            <Label class="text-xs text-muted-foreground">{{ t('settings.shortcut.cancel') }}</Label>
            <Select :model-value="store.cancelShortcut" @update:model-value="onCancelShortcutChange">
              <SelectTrigger class="w-full">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem
                  v-for="opt in cancelShortcutOptions"
                  :key="opt.value"
                  :value="opt.value"
                >
                  {{ t(opt.label) }}
                </SelectItem>
              </SelectContent>
            </Select>
          </div>
        </CardContent>
      </Card>

      <!-- Microphone -->
      <Card>
        <CardHeader class="pb-2 pt-3 px-4">
          <CardTitle class="text-sm font-medium">{{ t('settings.microphone') }}</CardTitle>
        </CardHeader>
        <CardContent class="px-4 pb-3 space-y-3">
          <Select
            :model-value="store.audioDevices.find(d => d.is_default)?.uid ?? '__default__'"
            @update:model-value="onDeviceChange"
          >
            <SelectTrigger class="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="__default__">
                {{ t('settings.microphone.default') }}
              </SelectItem>
              <SelectItem
                v-for="device in store.audioDevices"
                :key="device.uid"
                :value="device.uid"
              >
                {{ device.name }}
              </SelectItem>
            </SelectContent>
          </Select>

          <div class="flex items-center gap-2">
            <Button
              variant="outline"
              size="sm"
              @click="isTesting ? stopMicTest() : startMicTest()"
            >
              {{ isTesting ? t('settings.microphone.testing') : t('settings.microphone.test') }}
            </Button>
            <div v-if="isTesting" class="flex-1">
              <Progress :model-value="micLevel" class="h-2" />
            </div>
          </div>
        </CardContent>
      </Card>
    </div>
  </div>
</template>
