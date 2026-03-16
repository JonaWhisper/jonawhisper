import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useEnginesStore } from '@/stores/engines'
import type { Provider } from '@/stores/types'

export function useProviderModels(
  getProviderId: () => string,
  getModels: (provider: Provider, presets: ReturnType<typeof useEnginesStore>['providerPresets']) => string[],
) {
  const engines = useEnginesStore()
  const refreshing = ref(false)

  const selectedProvider = computed(() =>
    engines.providers.find(p => p.id === getProviderId())
  )

  const modelOptions = computed(() => {
    const provider = selectedProvider.value
    return provider ? getModels(provider, engines.providerPresets) : []
  })

  async function refreshModels() {
    const provider = selectedProvider.value
    if (!provider || refreshing.value) return
    refreshing.value = true
    try {
      const models = await invoke<string[]>('fetch_provider_models', { provider })
      await engines.updateProvider({ ...provider, cached_models: models })
    } catch (e) {
      console.error('refreshModels failed:', e)
    } finally {
      refreshing.value = false
    }
  }

  return { selectedProvider, modelOptions, refreshing, refreshModels }
}
