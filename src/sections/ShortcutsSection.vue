<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/settings'
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
  <div>
    <!-- Recording mode card -->
    <div class="wf-card">
      <div class="wf-card-title">{{ t('settings.shortcut.mode') }}</div>
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('settings.shortcut.mode') }}</div>
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
    <div class="wf-card">
      <div class="wf-card-title">{{ t('settings.shortcut.record') }}</div>
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('settings.shortcut.record') }}</div>
        </div>
        <ShortcutCapture
          :model-value="settings.hotkey"
          @update:model-value="onHotkeyChange"
        />
      </div>
      <div class="wf-form-row">
        <div>
          <div class="wf-form-label">{{ t('settings.shortcut.cancel') }}</div>
        </div>
        <div class="flex items-center gap-2">
          <ShortcutCapture
            :model-value="settings.cancelShortcut"
            @update:model-value="onCancelShortcutChange"
          />
          <Button
            variant="outline"
            size="sm"
            class="shrink-0 h-8 text-xs"
            @click="onDisableCancel"
          >
            {{ t('settings.shortcut.cancel.none') }}
          </Button>
        </div>
      </div>
    </div>
  </div>
</template>
