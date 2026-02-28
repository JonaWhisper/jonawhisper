<script setup lang="ts">
import { ref, computed, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { parseShortcut, formatShortcut, formatCaptureState, serializeShortcut, isDisabled, type ShortcutDef } from '@/utils/shortcut'

const props = defineProps<{
  modelValue: string
  disabled?: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()

const { t } = useI18n()

const capturing = ref(false)
const captureDisplay = ref('')

const parsed = computed(() => parseShortcut(props.modelValue))
const displayText = computed(() => {
  if (parsed.value && !isDisabled(parsed.value)) {
    return formatShortcut(parsed.value)
  }
  return t('settings.shortcut.cancel.none')
})

let unlistenUpdate: (() => void) | null = null
let unlistenComplete: (() => void) | null = null
let unlistenCancelled: (() => void) | null = null

async function startCapture() {
  if (props.disabled) return
  capturing.value = true
  captureDisplay.value = ''

  unlistenUpdate = await listen<{ modifiers: number; key_code: number | null }>('shortcut-capture-update', (event) => {
    captureDisplay.value = formatCaptureState(event.payload.modifiers, event.payload.key_code)
  })

  unlistenComplete = await listen<{ key_code: number; modifiers: number; kind: string; display: string }>('shortcut-capture-complete', (event) => {
    const shortcut: ShortcutDef = {
      key_code: event.payload.key_code,
      modifiers: event.payload.modifiers,
      kind: event.payload.kind as ShortcutDef['kind'],
    }
    emit('update:modelValue', serializeShortcut(shortcut))
    stopCapture()
  })

  unlistenCancelled = await listen('shortcut-capture-cancelled', () => {
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

function cleanup() {
  if (unlistenUpdate) { unlistenUpdate(); unlistenUpdate = null }
  if (unlistenComplete) { unlistenComplete(); unlistenComplete = null }
  if (unlistenCancelled) { unlistenCancelled(); unlistenCancelled = null }
}

onUnmounted(() => {
  if (capturing.value) {
    invoke('stop_shortcut_capture')
  }
  cleanup()
})
</script>

<template>
  <button
    type="button"
    class="flex items-center justify-between w-full h-9 rounded-md border px-3 py-2 text-sm transition-colors focus:outline-none"
    :class="[
      capturing
        ? 'border-primary ring-2 ring-primary/20 bg-primary/5'
        : 'border-border bg-background hover:bg-accent/50',
      disabled ? 'opacity-50 cursor-not-allowed' : 'cursor-pointer'
    ]"
    @click="capturing ? stopCapture() : startCapture()"
  >
    <span v-if="capturing" class="text-primary font-medium">
      {{ captureDisplay || t('shortcutCapture.waiting') }}
    </span>
    <span v-else class="text-foreground font-mono">
      {{ displayText }}
    </span>
    <span v-if="capturing" class="text-xs text-muted-foreground ml-2 shrink-0">
      {{ t('shortcutCapture.escToCancel') }}
    </span>
  </button>
</template>
