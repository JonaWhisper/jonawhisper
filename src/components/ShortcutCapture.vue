<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { parseShortcut, formatShortcutParts, formatCaptureState, serializeShortcut, isDisabled, type ShortcutDef } from '@/utils/shortcut'

const DISABLED_SHORTCUT: ShortcutDef = { key_codes: [], modifiers: 0, kind: 'Key' }

const props = defineProps<{
  modelValue: string
  disabled?: boolean
  fullWidth?: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const { t } = useI18n()

const capturing = ref(false)
const captureDisplay = ref('')

const parsed = computed(() => parseShortcut(props.modelValue))
const keyCaps = computed(() => {
  if (parsed.value && !isDisabled(parsed.value)) {
    return formatShortcutParts(parsed.value)
  }
  return []
})

const sideLabels = computed<Record<string, string>>(() => ({
  Right: t('shortcutCapture.side.right'),
  Left: t('shortcutCapture.side.left'),
}))

let unlistenUpdate: (() => void) | null = null
let unlistenComplete: (() => void) | null = null

async function startCapture() {
  if (props.disabled) return
  capturing.value = true
  captureDisplay.value = ''

  unlistenUpdate = await listen<{ modifiers: number; key_codes: number[] }>('shortcut-capture-update', (event) => {
    captureDisplay.value = formatCaptureState(event.payload.modifiers, event.payload.key_codes)
  })

  unlistenComplete = await listen<{ key_codes: number[]; modifiers: number; kind: string; display: string }>('shortcut-capture-complete', (event) => {
    const shortcut: ShortcutDef = {
      key_codes: event.payload.key_codes,
      modifiers: event.payload.modifiers,
      kind: event.payload.kind as ShortcutDef['kind'],
    }
    emit('update:modelValue', serializeShortcut(shortcut))
    stopCapture()
  })

  await invoke('start_shortcut_capture')
}

function stopCapture() {
  capturing.value = false
  captureDisplay.value = ''
  cleanup()
  invoke('stop_shortcut_capture')
}

function clearShortcut() {
  emit('update:modelValue', serializeShortcut(DISABLED_SHORTCUT))
}

function cleanup() {
  if (unlistenUpdate) { unlistenUpdate(); unlistenUpdate = null }
  if (unlistenComplete) { unlistenComplete(); unlistenComplete = null }
}

onUnmounted(() => {
  if (capturing.value) {
    invoke('stop_shortcut_capture')
  }
  cleanup()
})
</script>

<template>
  <div
    class="shortcut-capture"
    :class="{ capturing, disabled, 'full-width': fullWidth }"
    @click="!capturing && !disabled && startCapture()"
  >
    <!-- Capture mode: pulsing hint -->
    <template v-if="capturing">
      <span v-if="captureDisplay" class="shortcut-capture-keys">{{ captureDisplay }}</span>
      <span v-else class="shortcut-capture-hint">{{ t('shortcutCapture.waiting') }}</span>
    </template>

    <!-- Display mode: key caps -->
    <template v-else-if="keyCaps.length > 0">
      <span v-for="(part, i) in keyCaps" :key="i" class="key-cap">
        {{ part.symbol }}<span v-if="part.side" class="key-cap-side">{{ sideLabels[part.side] ?? part.side }}</span>
      </span>
    </template>

    <!-- Disabled state -->
    <span v-else class="shortcut-capture-none">{{ t('settings.shortcut.cancel.none') }}</span>

    <!-- Clear/Cancel button: cancel capture, or clear assigned shortcut -->
    <button
      v-if="capturing"
      type="button"
      class="shortcut-clear"
      @click.stop="stopCapture"
    >&times;</button>
    <button
      v-else-if="keyCaps.length > 0 && !disabled"
      type="button"
      class="shortcut-clear"
      @click.stop="clearShortcut"
    >&times;</button>
  </div>
</template>

<style scoped>
.shortcut-capture {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 5px 12px;
  background: hsl(var(--muted));
  border: 1.5px solid hsl(var(--border));
  border-radius: 6px;
  font-size: 13px;
  color: hsl(var(--foreground));
  cursor: pointer;
  min-width: 200px;
  height: 32px;
  justify-content: center;
  transition: all 0.2s;
}

.shortcut-capture.full-width {
  width: 100%;
  height: 36px;
}

.shortcut-capture:hover {
  background: hsl(var(--accent));
}

.shortcut-capture.disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.shortcut-capture.capturing {
  border-color: var(--panel-accent, #007AFF);
  background: rgba(0, 122, 255, 0.06);
  box-shadow: 0 0 0 3px rgba(0, 122, 255, 0.15);
}

.key-cap {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 1px 5px;
  background: hsl(var(--card));
  border: 0.5px solid hsl(var(--border));
  border-radius: 4px;
  font-size: 12px;
  font-weight: 500;
  min-width: 22px;
  box-shadow: 0 1px 1px rgba(0, 0, 0, 0.06);
}

.key-cap-side {
  font-size: 9px;
  font-weight: 400;
  opacity: 0.7;
  margin-left: 2px;
}

.shortcut-capture-hint {
  font-size: 11px;
  color: var(--panel-accent, #007AFF);
  animation: captureFlash 1s ease-in-out infinite;
}

.shortcut-capture-keys {
  font-size: 13px;
  font-weight: 500;
  color: var(--panel-accent, #007AFF);
}

.shortcut-capture-none {
  font-size: 12px;
  color: hsl(var(--muted-foreground));
}

.shortcut-clear {
  width: 16px;
  height: 16px;
  border: none;
  background: hsl(var(--muted-foreground) / 0.5);
  color: hsl(var(--card));
  cursor: pointer;
  border-radius: 50%;
  font-size: 11px;
  line-height: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: all 0.15s;
  flex-shrink: 0;
  margin-left: 2px;
}

.shortcut-clear:hover {
  background: hsl(var(--muted-foreground));
}

@keyframes captureFlash {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}
</style>
