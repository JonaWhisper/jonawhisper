<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEnginesStore } from '@/stores/engines'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Check } from 'lucide-vue-next'

const { t } = useI18n()
const engines = useEnginesStore()

let pollInterval: ReturnType<typeof setInterval> | null = null

const permissions = computed(() => engines.permissions)

const items = computed(() => [
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
  await engines.requestPermission(kind)
}

onMounted(() => {
  engines.fetchPermissions()
  pollInterval = setInterval(() => engines.fetchPermissions(), 1500)
})

onUnmounted(() => {
  if (pollInterval) {
    clearInterval(pollInterval)
    pollInterval = null
  }
})
</script>

<template>
  <div>
    <div class="text-[20px] font-bold tracking-[-0.02em] mb-4">{{ t('panel.permissions') }}</div>

    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="flex flex-col gap-2">
        <div
          v-for="item in items"
          :key="item.key"
          class="flex items-center gap-3 px-4 py-3 rounded-lg border border-border bg-card"
        >
          <div class="flex-1 min-w-0">
            <div class="text-sm font-medium leading-tight">{{ item.label }}</div>
            <div class="text-xs text-muted-foreground mt-0.5 leading-snug">{{ item.desc }}</div>
          </div>
          <Badge
            v-if="item.status === 'Granted'"
            variant="secondary"
            class="bg-green-500/10 text-green-500 border-transparent shrink-0 h-8 px-3"
          >
            <Check class="w-3 h-3 mr-1" />
            {{ t('setup.granted') }}
          </Badge>
          <Button v-else size="sm" class="shrink-0" @click="grant(item.key)">
            {{ t('setup.grant') }}
          </Button>
        </div>
      </div>
    </div>
  </div>
</template>
