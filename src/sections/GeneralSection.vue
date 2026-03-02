<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useSettingsStore } from '@/stores/settings'
import { i18n } from '@/main'
import { Label } from '@/components/ui/label'
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
  <div class="space-y-6">
    <!-- Appearance -->
    <div class="space-y-3">
      <h3 class="text-sm font-medium">{{ t('general.appearance') }}</h3>
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

    <!-- Interface Language -->
    <div class="space-y-2">
      <Label class="text-sm font-medium">{{ t('general.interfaceLanguage') }}</Label>
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

    <!-- About -->
    <div class="space-y-2">
      <h3 class="text-sm font-medium">{{ t('general.about') }}</h3>
      <div class="flex items-center gap-3 rounded-md border border-border p-3">
        <div class="flex items-center justify-center w-10 h-10 rounded-lg bg-primary/10 text-primary font-bold text-lg shrink-0">
          W
        </div>
        <div>
          <p class="text-sm font-medium">WhisperDictate</p>
          <p class="text-xs text-muted-foreground">Tauri + Rust + Vue</p>
        </div>
      </div>
    </div>
  </div>
</template>
