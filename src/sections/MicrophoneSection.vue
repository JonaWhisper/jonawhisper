<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useSettingsStore } from '@/stores/settings'
import { useEnginesStore } from '@/stores/engines'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import { Switch } from '@/components/ui/switch'
import { Slider } from '@/components/ui/slider'
import {
  Select, SelectContent, SelectItem, SelectTrigger,
} from '@/components/ui/select'
import SpectrumBars from '@/components/SpectrumBars.vue'
import { Laptop, Usb, Bluetooth, Waves, HardDrive, Zap, Monitor, Mic } from 'lucide-vue-next'
import type { Component } from 'vue'

const { t } = useI18n()
const settings = useSettingsStore()
const engines = useEnginesStore()

// Mic test
const isTesting = ref(false)
const testSpectrum = ref<number[]>(new Array(12).fill(0))
let spectrumUnlisten: (() => void) | null = null
let micTestStoppedUnlisten: (() => void) | null = null

const TRANSPORT_ICONS: Record<string, Component> = {
  BuiltIn: Laptop, USB: Usb, Bluetooth: Bluetooth,
  Virtual: Waves, Aggregate: HardDrive, Thunderbolt: Zap,
  HDMI: Monitor, Unknown: Mic,
}
function deviceIcon(type: string): Component { return TRANSPORT_ICONS[type] ?? Mic }

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

onMounted(async () => {
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
</template>
