<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/settings'
import ShortcutCapture from '@/components/ShortcutCapture.vue'
import SegmentedToggle from '@/components/SegmentedToggle.vue'

const { t } = useI18n()
const settings = useSettingsStore()

async function onRecordingModeChange(mode: string) {
  await settings.setSetting('recording_mode', mode)
}

async function onHotkeyChange(value: string) {
  await settings.setSetting('hotkey', value)
}

async function onCancelShortcutChange(value: string) {
  await settings.setSetting('cancel_shortcut', value)
}
</script>

<template>
  <div>
    <div class="text-[20px] font-bold tracking-[-0.02em] mb-4">{{ t('panel.shortcuts') }}</div>

    <!-- Recording mode card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.shortcut.mode') }}</div>
      <div class="flex items-center justify-between py-2 gap-3">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.shortcut.mode') }}</div>
        </div>
        <SegmentedToggle
          :model-value="settings.recordingMode"
          :options="[
            { value: 'push_to_talk', label: t('settings.shortcut.mode.pushToTalk') },
            { value: 'toggle', label: t('settings.shortcut.mode.toggle') },
          ]"
          @update:model-value="onRecordingModeChange"
        />
      </div>
    </div>

    <!-- Keyboard shortcuts card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.shortcut.record') }}</div>
      <div class="flex items-center justify-between py-2 gap-3">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.shortcut.record') }}</div>
        </div>
        <ShortcutCapture
          :model-value="settings.hotkey"
          @update:model-value="onHotkeyChange"
        />
      </div>
      <div class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
        <div>
          <div class="text-[13px] text-foreground">{{ t('settings.shortcut.cancel') }}</div>
        </div>
        <ShortcutCapture
          :model-value="settings.cancelShortcut"
          @update:model-value="onCancelShortcutChange"
        />
      </div>
    </div>
  </div>
</template>
