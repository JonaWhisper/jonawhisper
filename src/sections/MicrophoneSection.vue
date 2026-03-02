<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { useSettingsStore } from '@/stores/settings'
import { useEnginesStore } from '@/stores/engines'
import { Switch } from '@/components/ui/switch'
import { Slider } from '@/components/ui/slider'
import { Button } from '@/components/ui/button'
import {
  Select, SelectContent, SelectItem, SelectTrigger,
} from '@/components/ui/select'
import SpectrumBars from '@/components/SpectrumBars.vue'
import { Badge } from '@/components/ui/badge'
import { Laptop, Usb, Bluetooth, Waves, HardDrive, Zap, Monitor, Mic } from 'lucide-vue-next'
import type { Component } from 'vue'

const { t } = useI18n()
const settings = useSettingsStore()
const engines = useEnginesStore()

// Mic test
const isTesting = ref(false)
const testSpectrum = ref<number[]>(new Array(20).fill(0))
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

const micLevelBadge = computed(() => {
  if (!isTesting.value) return null
  const spectrum = testSpectrum.value
  const avg = spectrum.reduce((a, b) => a + b, 0) / spectrum.length
  // Values are 0..1 (clamped in Rust), smoothed in frontend
  // Silence: no badge. Weak: very faint signal. Good: normal speech. Saturated: near clipping.
  if (avg < 0.005) return null
  if (avg < 0.03) return { label: t('settings.microphone.level.weak'), cls: 'bg-orange-500/12 text-orange-600 dark:bg-orange-500/18 dark:text-orange-400' }
  if (avg < 0.6) return { label: t('settings.microphone.level.good'), cls: 'bg-emerald-500/12 text-emerald-600 dark:bg-emerald-500/18 dark:text-emerald-400' }
  return { label: t('settings.microphone.level.saturated'), cls: 'bg-red-500/10 text-red-600 dark:bg-red-500/18 dark:text-red-400' }
})

async function startMicTest() {
  if (isTesting.value) return
  isTesting.value = true
  testSpectrum.value = new Array(20).fill(0)
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
  testSpectrum.value = new Array(20).fill(0)
  if (spectrumUnlisten) {
    spectrumUnlisten()
    spectrumUnlisten = null
  }
  await invoke('stop_mic_test')
}

onMounted(async () => {
  micTestStoppedUnlisten = await listen('mic-test-stopped', () => {
    isTesting.value = false
    testSpectrum.value = new Array(20).fill(0)
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
  <div>
    <div class="section-title">{{ t('panel.microphone') }}</div>

    <!-- Input device card -->
    <div class="wf-card">
      <div class="wf-card-title">{{ t('settings.microphone') }}</div>
      <div class="wf-form-row">
        <div class="min-w-0 flex-1">
          <div class="wf-form-label">{{ t('settings.microphone') }}</div>
        </div>
        <Select
          :model-value="selectedDeviceUid"
          :disabled="engines.audioDevices.length === 0"
          @update:model-value="onDeviceChange"
        >
          <SelectTrigger class="w-auto min-w-[180px] h-8 text-xs">
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
      </div>
    </div>

    <!-- Mic test card -->
    <div class="wf-card">
      <div class="wf-card-title">{{ t('settings.microphone.test') }}</div>
      <div class="flex flex-col items-center gap-2.5">
        <div class="w-full flex justify-center">
          <SpectrumBars :spectrum="testSpectrum" size="md" />
        </div>
        <div class="flex items-center gap-2">
          <Button
            variant="default"
            size="sm"
            class="min-w-16"
            :disabled="engines.audioDevices.length === 0"
            @click="isTesting ? stopMicTest() : startMicTest()"
          >
            {{ isTesting ? t('settings.microphone.stop') : t('settings.microphone.test') }}
          </Button>
          <Badge
            v-if="micLevelBadge"
            variant="secondary"
            :class="['text-[10px] px-1.5 py-0.5 border-transparent font-medium transition-colors duration-300', micLevelBadge.cls]"
          >
            {{ micLevelBadge.label }}
          </Badge>
        </div>
      </div>
    </div>

    <!-- Audio ducking card -->
    <div class="wf-card">
      <div class="wf-card-title">{{ t('settings.microphone.ducking') }}</div>
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('settings.microphone.ducking') }}</div>
        </div>
        <Switch
          :model-value="settings.audioDuckingEnabled"
          @update:model-value="onAudioDuckingChange"
        />
      </div>
      <!-- Slider always visible, grayed when toggle off -->
      <div
        class="wf-form-row"
        :class="{ 'opacity-35 pointer-events-none': !settings.audioDuckingEnabled }"
      >
        <div>
          <div class="wf-form-label">{{ t('settings.microphone.duckingLevel') }}</div>
        </div>
        <div class="flex items-center gap-2">
          <Slider
            class="w-24"
            :model-value="[duckingSliderValue]"
            :min="5"
            :max="100"
            :step="5"
            @update:model-value="onDuckingSliderUpdate"
            @value-commit="onDuckingSliderCommit"
          />
          <span class="text-xs text-muted-foreground tabular-nums min-w-7 text-right">
            {{ duckingSliderValue >= 100 ? t('settings.microphone.duckingMute') : `${Math.round(duckingSliderValue)}%` }}
          </span>
        </div>
      </div>
    </div>
  </div>
</template>
