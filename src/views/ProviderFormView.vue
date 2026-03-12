<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { emit } from '@tauri-apps/api/event'
import { getCurrentWindow } from '@tauri-apps/api/window'
import type { Provider } from '@/stores/types'
import { useEnginesStore } from '@/stores/engines'
import ProviderForm from '@/components/ProviderForm.vue'

const { t } = useI18n()
const engines = useEnginesStore()

const providerId = new URLSearchParams(window.location.search).get('id')
const editProvider = ref<Provider | undefined>(undefined)
const ready = ref(false)

onMounted(async () => {
  // Load presets (needed by ProviderForm)
  await engines.fetchProviderPresets()

  if (providerId) {
    // Fetch all providers and find the one to edit
    await engines.fetchProviders()
    editProvider.value = engines.providers.find(p => p.id === providerId)
  }

  ready.value = true
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
  <div class="h-full bg-background p-5 overflow-y-auto">
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
