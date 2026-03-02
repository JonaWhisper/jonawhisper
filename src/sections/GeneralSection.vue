<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore } from '@/stores/settings'
import { i18n } from '@/main'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import SegmentedToggle from '@/components/SegmentedToggle.vue'

const { t } = useI18n()
const settings = useSettingsStore()

const localeOptions = [
  { value: 'auto', label: 'settings.locale.auto' },
  { value: 'fr', label: 'settings.locale.fr' },
  { value: 'en', label: 'settings.locale.en' },
]

async function onThemeChange(value: string) {
  await settings.setSetting('theme', value)
}

async function onLocaleChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.setSetting('app_locale', value)
  const locale = await invoke<string>('get_system_locale')
  i18n.global.locale.value = locale as 'fr' | 'en'
}
</script>

<template>
  <div>
    <!-- Appearance card -->
    <div class="wf-card">
      <div class="wf-card-title">{{ t('general.appearance') }}</div>
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('general.appearance') }}</div>
        </div>
        <SegmentedToggle
          :model-value="settings.theme"
          :options="[
            { value: 'system', label: t('general.theme.system') },
            { value: 'light', label: t('general.theme.light') },
            { value: 'dark', label: t('general.theme.dark') },
          ]"
          @update:model-value="onThemeChange"
        />
      </div>
    </div>

    <!-- Interface language card -->
    <div class="wf-card">
      <div class="wf-card-title">{{ t('general.interfaceLanguage') }}</div>
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('general.interfaceLanguage') }}</div>
        </div>
        <Select :model-value="settings.appLocale" @update:model-value="onLocaleChange">
          <SelectTrigger class="w-auto min-w-[130px] h-8 text-xs">
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

    <!-- About card -->
    <div class="wf-card" style="padding: 20px;">
      <div class="text-center">
        <div class="wf-about-icon">W</div>
        <div class="text-base font-bold">WhisperDictate</div>
        <div class="text-xs text-muted-foreground mt-0.5">Tauri v2 — Rust + Vue</div>
        <div class="mt-3 flex gap-4 justify-center">
          <a class="text-xs text-[var(--panel-accent)] hover:underline cursor-pointer">GitHub</a>
          <a class="text-xs text-[var(--panel-accent)] hover:underline cursor-pointer">Licence MIT</a>
        </div>
      </div>
    </div>
  </div>
</template>
