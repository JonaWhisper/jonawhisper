<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { getVersion } from '@tauri-apps/api/app'
import { useSettingsStore } from '@/stores/settings'
import { i18n } from '@/main'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import SegmentedToggle from '@/components/SegmentedToggle.vue'

const { t } = useI18n()
const settings = useSettingsStore()
const appVersion = ref('')

// "disabled" | "requires_approval" | "enabled"
const launchAtLoginStatus = ref<string>('disabled')
const launchAtLoginError = ref<string>('')
const launchAtLoginPending = ref(false)

const launchAtLoginEnabled = computed(() => launchAtLoginStatus.value !== 'disabled')

onMounted(async () => {
  appVersion.value = await getVersion()
  launchAtLoginStatus.value = await invoke<string>('get_launch_at_login_status')
})

async function onLaunchAtLoginChange(checked: boolean) {
  if (launchAtLoginPending.value) return
  launchAtLoginPending.value = true
  launchAtLoginError.value = ''
  try {
    launchAtLoginStatus.value = await invoke<string>('set_launch_at_login', { enabled: checked })
  } catch (e) {
    launchAtLoginError.value = String(e)
    console.error('set_launch_at_login error:', e)
  } finally {
    launchAtLoginPending.value = false
  }
}

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
    <div class="text-[20px] font-bold tracking-[-0.02em] mb-4">{{ t('panel.general') }}</div>

    <!-- Launch at Login card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('general.launchAtLogin') }}</div>
      <div class="flex items-center justify-between py-2 gap-3">
        <div class="flex-1 min-w-0">
          <div class="text-[13px] text-foreground">{{ t('general.launchAtLogin.desc') }}</div>
          <div v-if="launchAtLoginStatus === 'requires_approval'" class="text-[11px] text-amber-500 mt-0.5">{{ t('general.launchAtLogin.requiresApproval') }}</div>
          <div v-if="launchAtLoginError" class="text-[11px] text-red-500 mt-0.5">{{ launchAtLoginError }}</div>
        </div>
        <Switch :model-value="launchAtLoginEnabled" :disabled="launchAtLoginPending" @update:model-value="onLaunchAtLoginChange" />
      </div>
    </div>

    <!-- Appearance card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('general.appearance') }}</div>
      <div class="flex items-center justify-between py-2 gap-3">
        <div>
          <div class="text-[13px] text-foreground">{{ t('general.appearance') }}</div>
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
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('general.interfaceLanguage') }}</div>
      <div class="flex items-center justify-between py-2 gap-3">
        <div>
          <div class="text-[13px] text-foreground">{{ t('general.interfaceLanguage') }}</div>
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
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-5 mb-2.5">
      <div class="text-center">
        <div class="w-12 h-12 mx-auto mb-2 bg-gradient-to-br from-panel-accent to-[#5856d6] rounded-xl flex items-center justify-center text-[22px] font-bold text-white">J</div>
        <div class="text-base font-bold">JonaWhisper</div>
        <div v-if="appVersion" class="text-xs text-muted-foreground mt-0.5">v{{ appVersion }}</div>
        <div class="text-[10px] text-muted-foreground/60 mt-1">GPL-3.0</div>
      </div>
    </div>
  </div>
</template>
