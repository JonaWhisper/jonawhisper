<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/settings'
import { Label } from '@/components/ui/label'
import { Button } from '@/components/ui/button'
import ShortcutCapture from '@/components/ShortcutCapture.vue'
import SegmentedToggle from '@/components/SegmentedToggle.vue'
import { serializeShortcut } from '@/utils/shortcut'

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

function onDisableCancel() {
  const disabled = serializeShortcut({ key_code: 0, modifiers: 0, kind: 'Key' })
  onCancelShortcutChange(disabled)
}
</script>

<template>
  <div class="space-y-4">
    <div class="flex items-center justify-between">
      <Label class="text-sm font-medium">{{ t('settings.shortcut.mode') }}</Label>
      <SegmentedToggle
        :model-value="settings.recordingMode"
        :options="[
          { value: 'push_to_talk', label: t('settings.shortcut.mode.pushToTalk') },
          { value: 'toggle', label: t('settings.shortcut.mode.toggle') },
        ]"
        @update:model-value="onRecordingModeChange"
      />
    </div>

    <div class="space-y-2">
      <Label class="text-sm font-medium">{{ t('settings.shortcut.record') }}</Label>
      <ShortcutCapture
        :model-value="settings.hotkey"
        @update:model-value="onHotkeyChange"
      />
    </div>

    <div class="space-y-2">
      <Label class="text-sm font-medium">{{ t('settings.shortcut.cancel') }}</Label>
      <div class="flex gap-2">
        <ShortcutCapture
          class="flex-1"
          :model-value="settings.cancelShortcut"
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
</template>
