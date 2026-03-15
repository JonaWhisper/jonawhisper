<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { getVersion } from '@tauri-apps/api/app'
import { FolderOpen, RefreshCw, Download } from 'lucide-vue-next'
import { useSettingsStore } from '@/stores/settings'
import { useAppStore } from '@/stores/app'
import i18n from '@/i18n'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import SegmentedToggle from '@/components/SegmentedToggle.vue'

const { t } = useI18n()
const settings = useSettingsStore()
const app = useAppStore()
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

    <!-- Launch at Login card (hidden when not signed with Developer ID) -->
    <div
      v-if="launchAtLoginStatus !== 'unavailable'"
      class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5"
    >
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

    <!-- Log level card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('general.logging') }}</div>
      <div class="flex items-center justify-between py-2 gap-3">
        <div>
          <div class="text-[13px] text-foreground">{{ t('general.logLevel') }}</div>
        </div>
        <div class="flex items-center gap-2">
          <Select :model-value="settings.logLevel" @update:model-value="(v) => settings.setSetting('log_level', String(v))">
            <SelectTrigger class="w-auto min-w-[110px] h-8 text-xs">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value="error">{{ t('logLevel.error') }}</SelectItem>
              <SelectItem value="warn">{{ t('logLevel.warn') }}</SelectItem>
              <SelectItem value="info">{{ t('logLevel.info') }}</SelectItem>
              <SelectItem value="debug">{{ t('logLevel.debug') }}</SelectItem>
            </SelectContent>
          </Select>
          <TooltipProvider :delay-duration="200">
            <Tooltip>
              <TooltipTrigger as-child>
                <button
                  :aria-label="t('general.openLogs')"
                  class="h-8 w-8 flex items-center justify-center rounded-md border border-input text-muted-foreground hover:text-foreground hover:bg-accent transition-colors"
                  @click="invoke('open_logs_folder')"
                >
                  <FolderOpen class="w-4 h-4" />
                </button>
              </TooltipTrigger>
              <TooltipContent side="bottom" :side-offset="4">{{ t('general.openLogs') }}</TooltipContent>
            </Tooltip>
          </TooltipProvider>
        </div>
      </div>
      <div class="flex items-center justify-between py-2 gap-3">
        <div>
          <div class="text-[13px] text-foreground">{{ t('general.logRetention') }}</div>
        </div>
        <Select :model-value="settings.logRetention" @update:model-value="(v) => settings.setSetting('log_retention', String(v))">
          <SelectTrigger class="w-auto min-w-[160px] h-8 text-xs">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value="previous">{{ t('general.logRetention.previous') }}</SelectItem>
            <SelectItem value="3days">{{ t('general.logRetention.3days') }}</SelectItem>
            <SelectItem value="7days">{{ t('general.logRetention.7days') }}</SelectItem>
            <SelectItem value="30days">{{ t('general.logRetention.30days') }}</SelectItem>
            <SelectItem value="all">{{ t('general.logRetention.all') }}</SelectItem>
          </SelectContent>
        </Select>
      </div>
    </div>

    <!-- Update card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('general.update.title') }}</div>

      <!-- Update available -->
      <div v-if="app.updateAvailable" class="flex items-center justify-between py-2 gap-3">
        <div class="flex-1 min-w-0">
          <div class="text-[13px] text-emerald-500 font-medium">{{ t('general.update.available', { version: app.updateAvailable.version }) }}</div>
          <div v-if="app.updateAvailable.body" class="text-[11px] text-muted-foreground mt-0.5 line-clamp-2">{{ app.updateAvailable.body }}</div>
        </div>
        <button
          :disabled="app.updateInstalling"
          class="inline-flex items-center gap-1.5 px-3 h-8 rounded-md bg-emerald-600 text-white text-xs font-medium hover:bg-emerald-700 disabled:opacity-50 transition-colors shrink-0"
          @click="app.installUpdate()"
        >
          <Download v-if="!app.updateInstalling" class="w-3.5 h-3.5" />
          <RefreshCw v-else class="w-3.5 h-3.5 animate-spin" />
          {{ app.updateInstalling ? t('general.update.installing') : t('general.update.install') }}
        </button>
      </div>

      <!-- Checking -->
      <div v-else-if="app.updateChecking" class="flex items-center justify-between py-2 gap-3">
        <div class="text-[13px] text-muted-foreground">{{ t('general.update.checking') }}</div>
        <RefreshCw class="w-4 h-4 text-muted-foreground animate-spin shrink-0" />
      </div>

      <!-- Up to date / error -->
      <div v-else class="flex items-center justify-between py-2 gap-3">
        <div>
          <div class="text-[13px]" :class="app.updateError ? 'text-amber-500 font-medium' : 'text-foreground'">v{{ appVersion }}</div>
          <div v-if="!app.updateError" class="text-[11px] text-muted-foreground mt-0.5">{{ t('general.update.upToDate') }}</div>
          <div v-else class="text-[11px] text-amber-500/70 mt-0.5">{{ app.updateError }}</div>
        </div>
        <TooltipProvider :delay-duration="200">
          <Tooltip>
            <TooltipTrigger as-child>
              <button
                class="h-8 w-8 flex items-center justify-center rounded-md border border-input text-muted-foreground hover:text-foreground hover:bg-accent transition-colors shrink-0"
                @click="app.checkForUpdate()"
              >
                <RefreshCw class="w-4 h-4" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="bottom" :side-offset="4">{{ t('general.update.check') }}</TooltipContent>
          </Tooltip>
        </TooltipProvider>
      </div>

    </div>

    <!-- About card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-5 mb-2.5">
      <div class="text-center">
        <img src="@/assets/icon.png" alt="JonaWhisper" class="w-12 h-12 mx-auto mb-2 rounded-xl" />
        <div class="text-base font-bold">JonaWhisper</div>
        <div v-if="appVersion" class="text-xs text-muted-foreground mt-0.5">v{{ appVersion }}</div>
        <div class="text-[10px] text-muted-foreground/60 mt-1">GPL-3.0</div>
      </div>
    </div>
  </div>
</template>
