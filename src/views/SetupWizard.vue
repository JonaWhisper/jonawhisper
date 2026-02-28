<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore } from '@/stores/app'
import { Button } from '@/components/ui/button'
import { Check } from 'lucide-vue-next'
import { Badge } from '@/components/ui/badge'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'
import SetupStep2 from '@/components/SetupStep2.vue'

const { t } = useI18n()
const store = useAppStore()
const step = ref(1)
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

function goToStep2() {
  step.value = 2
}

function goToStep1() {
  step.value = 1
}

async function handleStart() {
  await store.startMonitoring()
}

// Resize window on step change
watch(step, async (newStep) => {
  const win = getCurrentWindow()
  if (newStep === 2) {
    // Stop polling permissions — no longer needed
    if (pollInterval) {
      clearInterval(pollInterval)
      pollInterval = null
    }
    await win.setSize(new LogicalSize(680, 480))
  } else {
    // Resume polling
    if (!pollInterval) {
      pollInterval = setInterval(() => store.fetchPermissions(), 1500)
    }
    await win.setSize(new LogicalSize(420, 420))
  }
})

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
  <div class="flex flex-col h-full">
    <!-- Step indicator -->
    <div class="flex items-center justify-center gap-2 pt-3">
      <div
        class="w-2 h-2 rounded-full transition-colors"
        :class="step === 1 ? 'bg-primary' : 'bg-muted-foreground/30'"
      />
      <div
        class="w-2 h-2 rounded-full transition-colors"
        :class="step === 2 ? 'bg-primary' : 'bg-muted-foreground/30'"
      />
    </div>

    <!-- Step 1: Permissions -->
    <div v-if="step === 1" class="flex flex-col flex-1 p-5 select-none">
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
            <Check class="w-3 h-3" />
            {{ t('setup.granted') }}
          </Badge>
          <Button v-else size="sm" @click="grant(item.key)">
            {{ t('setup.grant') }}
          </Button>
        </div>
      </div>

      <div class="mt-3 pb-2">
        <Button class="w-full" :disabled="!canContinue" @click="goToStep2">
          {{ t('setup.continue') }}
        </Button>
      </div>
    </div>

    <!-- Step 2: Configuration -->
    <SetupStep2
      v-if="step === 2"
      class="flex-1 min-h-0"
      @start="handleStart"
      @back="goToStep1"
    />
  </div>
</template>
