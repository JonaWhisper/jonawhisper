<script setup lang="ts">
import { ref, onMounted, onUnmounted, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { LogicalSize } from '@tauri-apps/api/dpi'
import type { Provider } from '@/stores/types'
import { useEnginesStore } from '@/stores/engines'
import ProviderForm from '@/components/ProviderForm.vue'

const { t } = useI18n()
const engines = useEnginesStore()

const providerId = new URLSearchParams(window.location.search).get('id')
const editProvider = ref<Provider | undefined>(undefined)
const ready = ref(false)
const container = ref<HTMLElement | null>(null)

const WIDTH = 420

function fitWindow() {
  if (!container.value) return
  const height = Math.min(container.value.scrollHeight, 800)
  getCurrentWindow().setSize(new LogicalSize(WIDTH, height))
}

let observer: ResizeObserver | null = null

onMounted(async () => {
  await engines.fetchProviderPresets()

  if (providerId) {
    await engines.fetchProviders()
    editProvider.value = engines.providers.find(p => p.id === providerId)
  }

  ready.value = true
  await nextTick()
  fitWindow()

  observer = new ResizeObserver(fitWindow)
  if (container.value) observer.observe(container.value)
})

onUnmounted(() => {
  observer?.disconnect()
})

async function onSave(provider: Provider) {
  if (providerId) {
    await invoke('update_provider', { provider })
  } else {
    await invoke('add_provider', { provider })
  }
  await emit('provider-saved')
  getCurrentWindow().close()
}

function onCancel() {
  getCurrentWindow().close()
}
</script>

<template>
  <div ref="container" class="bg-background p-5">
    <h2 class="text-base font-semibold mb-4">
      {{ providerId ? t('provider.editTitle') : t('settings.providers.add') }}
    </h2>
    <ProviderForm
      v-if="ready"
      :provider="editProvider"
      @save="onSave"
      @cancel="onCancel"
    />
  </div>
</template>
