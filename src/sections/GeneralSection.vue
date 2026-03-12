<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { getVersion } from '@tauri-apps/api/app'
import { FolderOpen, RefreshCw, Download } from 'lucide-vue-next'
import { useSettingsStore } from '@/stores/settings'
import i18n from '@/i18n'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import SegmentedToggle from '@/components/SegmentedToggle.vue'

const { t } = useI18n()
const settings = useSettingsStore()
const appVersion = ref('')

// "disabled" | "requires_approval" | "enabled"
const launchAtLoginStatus = ref<string>('disabled')
const launchAtLoginError = ref<string>('')
const launchAtLoginPending = ref(false)

const launchAtLoginEnabled = computed(() => launchAtLoginStatus.value !== 'disabled')

// App update state
const updateAvailable = ref<{ version: string; body: string | null } | null>(null)
const updateChecking = ref(false)
const updateInstalling = ref(false)
const updateError = ref('')

async function checkForUpdate() {
  updateChecking.value = true
  updateError.value = ''
  try {
    const result = await invoke<{ version: string; body: string | null } | null>('check_for_update')
    updateAvailable.value = result
  } catch (e) {
    updateError.value = String(e)
  } finally {
    updateChecking.value = false
  }
}

async function installUpdate() {
  updateInstalling.value = true
  updateError.value = ''
  try {
    await invoke('install_update')
    // App will restart automatically
  } catch (e) {
    updateError.value = String(e)
    updateInstalling.value = false
  }
}

onMounted(async () => {
  appVersion.value = await getVersion()
  launchAtLoginStatus.value = await invoke<string>('get_launch_at_login_status')
  checkForUpdate()
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
              <SelectItem value="error">Error</SelectItem>
              <SelectItem value="warn">Warning</SelectItem>
              <SelectItem value="info">Info</SelectItem>
              <SelectItem value="debug">Debug</SelectItem>
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

    <!-- About card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-5 mb-2.5">
      <div class="text-center">
        <img src="@/assets/icon.png" alt="JonaWhisper" class="w-12 h-12 mx-auto mb-2 rounded-xl" />
        <div class="text-base font-bold">JonaWhisper</div>
        <div v-if="appVersion" class="text-xs text-muted-foreground mt-0.5">v{{ appVersion }}</div>
        <div class="text-[10px] text-muted-foreground/60 mt-1">GPL-3.0</div>

        <!-- Update section -->
        <div class="mt-3">
          <div v-if="updateAvailable" class="flex flex-col items-center gap-1.5">
            <div class="text-xs text-emerald-500">{{ t('general.update.available', { version: updateAvailable.version }) }}</div>
            <button
              :disabled="updateInstalling"
              class="inline-flex items-center gap-1.5 px-3 h-7 rounded-md bg-emerald-600 text-white text-xs font-medium hover:bg-emerald-700 disabled:opacity-50 transition-colors"
              @click="installUpdate"
            >
              <Download v-if="!updateInstalling" class="w-3.5 h-3.5" />
              <RefreshCw v-else class="w-3.5 h-3.5 animate-spin" />
              {{ updateInstalling ? t('general.update.installing') : t('general.update.install') }}
            </button>
          </div>
          <div v-else-if="updateChecking" class="flex items-center justify-center gap-1.5 text-xs text-muted-foreground">
            <RefreshCw class="w-3 h-3 animate-spin" />
            {{ t('general.update.checking') }}
          </div>
          <div v-else class="flex items-center justify-center gap-1.5">
            <span class="text-xs text-muted-foreground">{{ t('general.update.upToDate') }}</span>
            <TooltipProvider :delay-duration="200">
              <Tooltip>
                <TooltipTrigger as-child>
                  <button
                    class="h-5 w-5 flex items-center justify-center rounded text-muted-foreground/60 hover:text-foreground transition-colors"
                    @click="checkForUpdate"
                  >
                    <RefreshCw class="w-3 h-3" />
                  </button>
                </TooltipTrigger>
                <TooltipContent side="bottom" :side-offset="4">{{ t('general.update.check') }}</TooltipContent>
              </Tooltip>
            </TooltipProvider>
          </div>
          <div v-if="updateError" class="text-[11px] text-red-500 mt-1">{{ updateError }}</div>
        </div>
      </div>
    </div>
  </div>
</template>
