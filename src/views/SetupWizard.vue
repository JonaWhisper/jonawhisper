<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '../stores/app'
import { getCurrentWindow } from '@tauri-apps/api/window'

const { t } = useI18n()
const store = useAppStore()
let pollInterval: ReturnType<typeof setInterval> | null = null

const permissions = computed(() => store.permissions)

const canContinue = computed(() =>
  permissions.value.microphone === 'Granted' &&
  permissions.value.accessibility === 'Granted' &&
  permissions.value.input_monitoring === 'Granted'
)

const permissionItems = computed(() => [
  {
    key: 'microphone',
    label: t('setup.microphone'),
    desc: t('setup.microphone.desc'),
    status: permissions.value.microphone,
  },
  {
    key: 'accessibility',
    label: t('setup.accessibility'),
    desc: t('setup.accessibility.desc'),
    status: permissions.value.accessibility,
  },
  {
    key: 'input_monitoring',
    label: t('setup.inputMonitoring'),
    desc: t('setup.inputMonitoring.desc'),
    status: permissions.value.input_monitoring,
  },
])

async function grant(kind: string) {
  await store.requestPermission(kind)
}

async function handleContinue() {
  await store.startMonitoring()
  const win = getCurrentWindow()
  await win.close()
}

onMounted(() => {
  store.fetchPermissions()
  pollInterval = setInterval(() => {
    store.fetchPermissions()
  }, 1500)
})

onUnmounted(() => {
  if (pollInterval) clearInterval(pollInterval)
})
</script>

<template>
  <div class="flex flex-col h-screen p-5 select-none">
    <div class="text-center mb-4">
      <h1 class="text-lg font-bold">{{ t('setup.title') }}</h1>
      <p class="text-xs text-muted-foreground mt-0.5">{{ t('setup.subtitle') }}</p>
    </div>

    <div class="flex-1 flex flex-col justify-center gap-2.5">
      <div
        v-for="item in permissionItems"
        :key="item.key"
        class="flex items-center gap-3 px-3.5 py-2.5 rounded-lg border border-border bg-card"
      >
        <div class="flex-1 min-w-0">
          <div class="text-sm font-medium leading-tight">{{ item.label }}</div>
          <div class="text-[11px] text-muted-foreground mt-0.5 leading-snug">{{ item.desc }}</div>
        </div>
        <span
          v-if="item.status === 'Granted'"
          class="shrink-0 inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-[11px] font-medium bg-green-500/10 text-green-500"
        >
          <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor"><path d="M13.78 4.22a.75.75 0 010 1.06l-7.25 7.25a.75.75 0 01-1.06 0L2.22 9.28a.75.75 0 011.06-1.06L6 10.94l6.72-6.72a.75.75 0 011.06 0z"/></svg>
          {{ t('setup.granted') }}
        </span>
        <button
          v-else
          @click="grant(item.key)"
          class="shrink-0 px-3 py-1 text-xs font-medium rounded-md bg-primary text-primary-foreground hover:bg-primary/90 active:scale-95 transition-all"
        >
          {{ t('setup.grant') }}
        </button>
      </div>
    </div>

    <div class="mt-3 space-y-2">
      <p class="text-[11px] text-muted-foreground text-center leading-snug">{{ t('setup.note') }}</p>
      <button
        @click="handleContinue"
        :disabled="!canContinue"
        class="w-full py-2 px-4 rounded-md text-sm font-medium transition-all"
        :class="canContinue
          ? 'bg-primary text-primary-foreground hover:bg-primary/90 active:scale-[0.98]'
          : 'bg-muted text-muted-foreground cursor-not-allowed'"
      >
        {{ t('setup.continue') }}
      </button>
    </div>
  </div>
</template>
