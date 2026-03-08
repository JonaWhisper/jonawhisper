import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import { hasAsrSupport, hasLlmSupport } from '@/config/providers'
import { useSettingsStore } from './settings'
import { isModelAvailable, parseCloudId } from './types'
import type { AudioDevice, EngineInfo, ASRModel, Language, Provider, PermissionReport, CleanupModel, AsrModelOption } from './types'

export const useEnginesStore = defineStore('engines', () => {
  const settingsStore = useSettingsStore()

  // State
  const engines = ref<EngineInfo[]>([])
  const models = ref<ASRModel[]>([])
  const languages = ref<Language[]>([])
  const audioDevices = ref<AudioDevice[]>([])
  const providers = ref<Provider[]>([])
  const permissions = ref<PermissionReport>({ microphone: 'Undetermined', accessibility: 'Denied', input_monitoring: 'Denied' })
  const updatableModelIds = ref<Set<string>>(new Set())

  // Computed
  const downloadedModels = computed(() => models.value.filter(isModelAvailable))
  const asrEngines = computed(() => engines.value.filter(e => e.category === 'asr'))
  const llmEngines = computed(() => engines.value.filter(e => e.category === 'llm'))
  const downloadedLlmModels = computed(() => {
    const llmEngineIds = new Set(llmEngines.value.map(e => e.id))
    return models.value.filter(m => llmEngineIds.has(m.engine_id) && m.is_downloaded)
  })
  const punctuationEngines = computed(() => engines.value.filter(e => e.category === 'punctuation'))
  const downloadedPunctuationModels = computed(() => {
    const ids = new Set(punctuationEngines.value.map(e => e.id))
    return models.value.filter(m => ids.has(m.engine_id) && m.is_downloaded)
  })
  const correctionEngines = computed(() => engines.value.filter(e => e.category === 'correction'))
  const downloadedCorrectionModels = computed(() => {
    const ids = new Set(correctionEngines.value.map(e => e.id))
    return models.value.filter(m => ids.has(m.engine_id) && m.is_downloaded)
  })
  const spellcheckEngines = computed(() => engines.value.filter(e => e.category === 'spellcheck'))
  const hasSpellcheckDict = computed(() => {
    const ids = new Set(spellcheckEngines.value.map(e => e.id))
    return models.value.some(m => ids.has(m.engine_id) && m.is_downloaded)
  })
  const punctuationModels = computed<CleanupModel[]>(() => {
    return downloadedPunctuationModels.value.map(m => ({
      id: m.id, label: m.label, group: 'punctuation' as const,
      params: m.params, ram: m.ram, lang_codes: m.lang_codes,
      quantization: m.quantization, recommended: !!m.recommended,
    }))
  })
  const cleanupModels = computed<CleanupModel[]>(() => {
    const result: CleanupModel[] = []
    for (const m of downloadedCorrectionModels.value) {
      result.push({ id: m.id, label: m.label, group: 'correction', params: m.params, ram: m.ram, lang_codes: m.lang_codes, quantization: m.quantization, recommended: !!m.recommended })
    }
    for (const m of downloadedLlmModels.value) {
      result.push({ id: m.id, label: m.label, group: 'llm', params: m.params, ram: m.ram, lang_codes: m.lang_codes, quantization: m.quantization, recommended: !!m.recommended })
    }
    for (const p of providers.value) {
      if (hasLlmSupport(p)) {
        result.push({ id: `cloud:${p.id}`, label: p.name, group: 'cloud', params: null, ram: null, lang_codes: null, quantization: null, recommended: false })
      }
    }
    return result
  })
  const asrModels = computed<AsrModelOption[]>(() => {
    const result: AsrModelOption[] = []
    const asrIds = new Set(asrEngines.value.map(e => e.id))
    for (const m of models.value) {
      if (!asrIds.has(m.engine_id)) continue
      if (isModelAvailable(m)) {
        result.push({ id: m.id, label: m.label, group: 'local', params: m.params, ram: m.ram, wer: m.wer, rtf: m.rtf, lang_codes: m.lang_codes, quantization: m.quantization, recommended: !!m.recommended })
      }
    }
    for (const p of providers.value) {
      if (hasAsrSupport(p)) {
        result.push({ id: `cloud:${p.id}`, label: p.name, group: 'cloud', params: null, ram: null, wer: null, rtf: null, lang_codes: null, quantization: null, recommended: false })
      }
    }
    return result
  })
  const isCloudAsr = computed(() => !!parseCloudId(settingsStore.selectedModelId))
  const asrCloudProviderId = computed(() => parseCloudId(settingsStore.selectedModelId) ?? '')
  const isCloudLlm = computed(() => !!parseCloudId(settingsStore.cleanupModelId))
  const isLocalLlm = computed(() => settingsStore.cleanupModelId.startsWith('llama:'))
  const cleanupCloudProviderId = computed(() => parseCloudId(settingsStore.cleanupModelId) ?? '')

  // Actions
  async function fetchEngines() {
    try { engines.value = await invoke('get_engines') }
    catch (e) { console.error('fetchEngines failed:', e) }
  }

  async function fetchModels() {
    try {
      models.value = await invoke('get_models')
      validateSelections()
    }
    catch (e) { console.error('fetchModels failed:', e) }
  }

  /** Reset selected model IDs if they point to models that no longer exist. */
  function validateSelections() {
    const asrIds = new Set(asrModels.value.map(m => m.id))
    if (settingsStore.selectedModelId && !asrIds.has(settingsStore.selectedModelId)) {
      const first = asrModels.value[0]
      settingsStore.setSetting('selected_model_id', first?.id ?? '')
    }
    const punctIds = new Set(punctuationModels.value.map(m => m.id))
    if (settingsStore.punctuationModelId && !punctIds.has(settingsStore.punctuationModelId)) {
      settingsStore.setSetting('punctuation_model_id', '')
    }
    const cleanupIds = new Set(cleanupModels.value.map(m => m.id))
    if (settingsStore.cleanupModelId && !cleanupIds.has(settingsStore.cleanupModelId)) {
      settingsStore.setSetting('cleanup_model_id', '')
    }
  }

  async function fetchLanguages() {
    try { languages.value = await invoke('get_languages') }
    catch (e) { console.error('fetchLanguages failed:', e) }
  }

  async function fetchAudioDevices() {
    try { audioDevices.value = await invoke('get_audio_devices') }
    catch (e) { console.error('fetchAudioDevices failed:', e) }
  }

  async function fetchPermissions() {
    try { permissions.value = await invoke('get_permissions') }
    catch (e) { console.error('fetchPermissions failed:', e) }
  }

  async function fetchProviders() {
    try { providers.value = await invoke('get_providers') }
    catch (e) { console.error('fetchProviders failed:', e) }
  }

  async function requestPermission(kind: string) {
    try { await invoke('request_permission', { kind }) }
    catch (e) { console.error('requestPermission failed:', e) }
  }

  async function addProvider(provider: Provider) {
    try {
      await invoke('add_provider', { provider })
      await fetchProviders()
    } catch (e) { console.error('addProvider failed:', e) }
  }

  async function removeProvider(id: string) {
    try {
      await invoke('remove_provider', { id })
      await fetchProviders()
    } catch (e) { console.error('removeProvider failed:', e) }
  }

  async function updateProvider(provider: Provider) {
    try {
      await invoke('update_provider', { provider })
      await fetchProviders()
    } catch (e) { console.error('updateProvider failed:', e) }
  }

  function hasUpdate(modelId: string): boolean {
    return updatableModelIds.value.has(modelId)
  }

  // Listeners
  function setupListeners() {
    listen('permission-changed', () => {
      fetchPermissions()
    })

    listen('models-changed', () => {
      fetchModels()
    })

    listen<string[]>('model-updates-available', (event) => {
      updatableModelIds.value = new Set(event.payload)
    })
  }

  return {
    engines, models, languages, audioDevices, providers, permissions,
    downloadedModels, asrEngines, llmEngines,
    punctuationEngines,
    correctionEngines,
    spellcheckEngines, hasSpellcheckDict,
    punctuationModels, cleanupModels, asrModels,
    isCloudAsr, asrCloudProviderId, isCloudLlm, isLocalLlm, cleanupCloudProviderId,
    updatableModelIds, hasUpdate,
    fetchEngines, fetchModels, fetchLanguages, fetchAudioDevices,
    fetchPermissions, fetchProviders,
    requestPermission, addProvider, removeProvider, updateProvider,
    setupListeners,
  }
})
