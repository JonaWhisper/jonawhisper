<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'

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
        <Badge v-if="item.status === 'Granted'" variant="secondary" class="bg-green-500/10 text-green-500 border-transparent">
          <svg class="w-3 h-3" viewBox="0 0 16 16" fill="currentColor"><path d="M13.78 4.22a.75.75 0 010 1.06l-7.25 7.25a.75.75 0 01-1.06 0L2.22 9.28a.75.75 0 011.06-1.06L6 10.94l6.72-6.72a.75.75 0 011.06 0z"/></svg>
          {{ t('setup.granted') }}
        </Badge>
        <Button v-else size="sm" @click="grant(item.key)">
          {{ t('setup.grant') }}
        </Button>
      </div>
    </div>

    <div class="mt-3 space-y-2">
      <p class="text-[11px] text-muted-foreground text-center leading-snug">{{ t('setup.note') }}</p>
      <Button class="w-full" :disabled="!canContinue" @click="handleContinue">
        {{ t('setup.continue') }}
      </Button>
    </div>
  </div>
</template>
