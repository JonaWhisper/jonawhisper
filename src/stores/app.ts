import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type Event } from '@tauri-apps/api/event'
import { hasAsrSupport } from '@/config/providers'
import { useSettingsStore } from './settings'
import { useHistoryStore } from './history'

// Re-export all types for backward compatibility
export type {
  AudioDevice, EngineInfo, ASRModel, Language, HistoryEntry,
  ProviderKind, Provider, PermissionReport, SettingsPayload,
} from './types'

import type {
  AudioDevice, EngineInfo, ASRModel, Language,
  Provider, PermissionReport,
  RecordingStoppedPayload, TranscriptionStartedPayload,
  TranscriptionCompletePayload, DownloadProgressPayload,
  AppStatePayload,
} from './types'

export const useAppStore = defineStore('app', () => {
  const settingsStore = useSettingsStore()
  const historyStore = useHistoryStore()

  // State
  const isRecording = ref(false)
  const isTranscribing = ref(false)
  const queueCount = ref(0)
  const activeDownloads = ref<Record<string, { progress: number; stopping: boolean; downloaded: number; totalSize: number; speed: number }>>({})
  const deletingModels = ref<Record<string, boolean>>({})
  const engines = ref<EngineInfo[]>([])
  const models = ref<ASRModel[]>([])
  const languages = ref<Language[]>([])
  const audioDevices = ref<AudioDevice[]>([])
  const permissions = ref<PermissionReport>({ microphone: 'Undetermined', accessibility: 'Denied', input_monitoring: 'Denied' })
  const providers = ref<Provider[]>([])

  // Proxy refs for backward compatibility (delegates to settings store)
  const selectedModelId = computed({ get: () => settingsStore.selectedModelId, set: v => { settingsStore.selectedModelId = v } })
  const selectedLanguage = computed({ get: () => settingsStore.selectedLanguage, set: v => { settingsStore.selectedLanguage = v } })
  const asrCloudModel = computed({ get: () => settingsStore.asrCloudModel, set: v => { settingsStore.asrCloudModel = v } })
  const textCleanupEnabled = computed({ get: () => settingsStore.textCleanupEnabled, set: v => { settingsStore.textCleanupEnabled = v } })
  const cleanupModelId = computed({ get: () => settingsStore.cleanupModelId, set: v => { settingsStore.cleanupModelId = v } })
  const llmModel = computed({ get: () => settingsStore.llmModel, set: v => { settingsStore.llmModel = v } })
  const llmMaxTokens = computed({ get: () => settingsStore.llmMaxTokens, set: v => { settingsStore.llmMaxTokens = v } })
  const hallucinationFilterEnabled = computed({ get: () => settingsStore.hallucinationFilterEnabled, set: v => { settingsStore.hallucinationFilterEnabled = v } })
  const selectedInputDeviceUid = computed({ get: () => settingsStore.selectedInputDeviceUid, set: v => { settingsStore.selectedInputDeviceUid = v } })
  const audioDuckingEnabled = computed({ get: () => settingsStore.audioDuckingEnabled, set: v => { settingsStore.audioDuckingEnabled = v } })
  const audioDuckingLevel = computed({ get: () => settingsStore.audioDuckingLevel, set: v => { settingsStore.audioDuckingLevel = v } })
  const gpuMode = computed({ get: () => settingsStore.gpuMode, set: v => { settingsStore.gpuMode = v } })
  const hotkey = computed({ get: () => settingsStore.hotkey, set: v => { settingsStore.hotkey = v } })
  const cancelShortcut = computed({ get: () => settingsStore.cancelShortcut, set: v => { settingsStore.cancelShortcut = v } })
  const recordingMode = computed({ get: () => settingsStore.recordingMode, set: v => { settingsStore.recordingMode = v } })
  const appLocale = computed({ get: () => settingsStore.appLocale, set: v => { settingsStore.appLocale = v } })

  // Proxy ref for history backward compat
  const history = computed({ get: () => historyStore.history, set: v => { historyStore.history = v } })

  // Computed
  const isBusy = computed(() => isRecording.value || isTranscribing.value || queueCount.value > 0 || Object.keys(activeDownloads.value).length > 0)
  const selectedEngine = computed(() => {
    const model = models.value.find(m => m.id === settingsStore.selectedModelId)
    return model ? engines.value.find(e => e.id === model.engine_id) : null
  })
  const downloadedModels = computed(() => models.value.filter(m => {
    if (m.download_type.type === 'RemoteAPI' || m.download_type.type === 'System') return true
    return m.is_downloaded
  }))
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
  const bertModelReady = computed(() => downloadedPunctuationModels.value.length > 0)
  const cleanupModels = computed(() => {
    const result: { id: string; label: string; group: string }[] = []
    for (const m of downloadedPunctuationModels.value) {
      result.push({ id: m.id, label: m.label, group: 'bert' })
    }
    for (const m of downloadedLlmModels.value) {
      result.push({ id: m.id, label: m.label, group: 'llm' })
    }
    for (const p of providers.value) {
      result.push({ id: `cloud:${p.id}`, label: p.name, group: 'cloud' })
    }
    return result
  })
  const asrModels = computed(() => {
    const result: { id: string; label: string; group: string }[] = []
    const asrIds = new Set(asrEngines.value.map(e => e.id))
    for (const m of models.value) {
      if (!asrIds.has(m.engine_id)) continue
      if (m.download_type.type === 'System' || m.is_downloaded) {
        result.push({ id: m.id, label: m.label, group: 'local' })
      }
    }
    for (const p of providers.value) {
      if (hasAsrSupport(p)) {
        result.push({ id: `cloud:${p.id}`, label: p.name, group: 'cloud' })
      }
    }
    return result
  })
  const isCloudAsr = computed(() => settingsStore.selectedModelId.startsWith('cloud:'))
  const asrCloudProviderId = computed(() =>
    isCloudAsr.value ? settingsStore.selectedModelId.slice('cloud:'.length) : ''
  )
  const isCloudLlm = computed(() => settingsStore.cleanupModelId.startsWith('cloud:'))
  const isLocalLlm = computed(() => settingsStore.cleanupModelId.startsWith('llama:'))
  const cleanupCloudProviderId = computed(() =>
    isCloudLlm.value ? settingsStore.cleanupModelId.slice('cloud:'.length) : ''
  )

  // Actions
  async function fetchEngines() {
    try { engines.value = await invoke('get_engines') }
    catch (e) { console.error('fetchEngines failed:', e) }
  }

  async function fetchModels() {
    try { models.value = await invoke('get_models') }
    catch (e) { console.error('fetchModels failed:', e) }
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

  async function fetchState() {
    try {
      const state = await invoke<AppStatePayload>('get_app_state')
      isRecording.value = state.is_recording
      isTranscribing.value = state.is_transcribing
      queueCount.value = state.queue_count
      // Hydrate activeDownloads from backend (preserve stopping flags for entries we already track)
      const hydrated: typeof activeDownloads.value = {}
      for (const [id, progress] of Object.entries(state.active_downloads ?? {})) {
        hydrated[id] = { progress, stopping: activeDownloads.value[id]?.stopping ?? false, downloaded: 0, totalSize: 0, speed: 0 }
      }
      activeDownloads.value = hydrated
    } catch (e) { console.error('fetchState failed:', e) }
  }

  // Delegated settings actions
  const { fetchSettings, setSetting, selectModel, selectLanguageAction } = settingsStore

  async function downloadModel(id: string) {
    if (activeDownloads.value[id]) return false
    const model = models.value.find(m => m.id === id)
    const initialProgress = model?.partial_progress ?? 0
    activeDownloads.value = { ...activeDownloads.value, [id]: { progress: initialProgress, stopping: false, downloaded: 0, totalSize: model?.size ?? 0, speed: 0 } }
    try {
      const success = await invoke<boolean>('download_model_cmd', { id })
      return success
    } catch (e) {
      console.error('downloadModel failed:', e)
      return false
    } finally {
      const entry = activeDownloads.value[id]
      if (entry) {
        const lastProgress = entry.progress
        const m = models.value.find(m => m.id === id)
        if (m) {
          if (lastProgress >= 1) {
            m.is_downloaded = true
            m.partial_progress = null
          } else if (lastProgress > 0) {
            m.partial_progress = lastProgress
          } else {
            m.partial_progress = null
          }
        }
        const { [id]: _, ...rest } = activeDownloads.value
        activeDownloads.value = rest
      }
      fetchModels()
    }
  }

  async function pauseDownload(id: string) {
    const entry = activeDownloads.value[id]
    if (!entry) return
    const m = models.value.find(m => m.id === id)
    if (m && entry.progress > 0) {
      m.partial_progress = entry.progress
    }
    const { [id]: _, ...rest } = activeDownloads.value
    activeDownloads.value = rest
    try { await invoke('pause_download', { id }) }
    catch (e) { console.error('pauseDownload failed:', e) }
  }

  async function cancelDownload(id: string) {
    if (activeDownloads.value[id]) {
      activeDownloads.value = { ...activeDownloads.value, [id]: { ...activeDownloads.value[id], stopping: true } }
    } else {
      const m = models.value.find(m => m.id === id)
      if (m) m.partial_progress = null
    }
    try {
      await invoke('cancel_download', { id })
      await fetchModels()
    } catch (e) { console.error('cancelDownload failed:', e) }
  }

  async function deleteModel(id: string) {
    deletingModels.value = { ...deletingModels.value, [id]: true }
    try {
      const success = await invoke<boolean>('delete_model_cmd', { id })
      if (success) {
        await fetchModels()
      }
      return success
    } catch (e) {
      console.error('deleteModel failed:', e)
      return false
    } finally {
      const { [id]: _, ...rest } = deletingModels.value
      deletingModels.value = rest
    }
  }

  async function startMonitoring() {
    try { await invoke('start_monitoring') }
    catch (e) { console.error('startMonitoring failed:', e) }
  }

  // Delegated history actions
  const { fetchHistory, clearHistoryAction, searchHistory, deleteHistoryEntry, deleteHistoryDay } = historyStore

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

  // Event listeners
  function setupListeners() {
    listen('recording-started', () => {
      isRecording.value = true
    })

    listen<RecordingStoppedPayload>('recording-stopped', (event: Event<RecordingStoppedPayload>) => {
      isRecording.value = false
      if (event.payload?.queue_count !== undefined) {
        queueCount.value = event.payload.queue_count
      }
    })

    listen<TranscriptionStartedPayload>('transcription-started', (event: Event<TranscriptionStartedPayload>) => {
      isTranscribing.value = true
      if (event.payload?.queue_count !== undefined) {
        queueCount.value = event.payload.queue_count
      }
    })

    listen<TranscriptionCompletePayload>('transcription-complete', (event: Event<TranscriptionCompletePayload>) => {
      isTranscribing.value = false
      queueCount.value = Math.max(0, queueCount.value - 1)
      if (event.payload?.text) {
        historyStore.addEntry({
          text: event.payload.text,
          timestamp: Date.now() / 1000,
          model_id: settingsStore.selectedModelId,
          language: settingsStore.selectedLanguage,
          cleanup_model_id: event.payload.cleanup_model_id ?? '',
          hallucination_filter: event.payload.hallucination_filter ?? false,
        })
      }
    })

    listen('transcription-error', () => {
      isTranscribing.value = false
      queueCount.value = Math.max(0, queueCount.value - 1)
    })

    listen('transcription-cancelled', () => {
      isTranscribing.value = false
      queueCount.value = 0
    })

    listen<DownloadProgressPayload>('download-progress', (event: Event<DownloadProgressPayload>) => {
      const { model_id, progress, downloaded, total_size, speed } = event.payload ?? {}
      if (model_id && progress !== undefined && activeDownloads.value[model_id]) {
        const entry = activeDownloads.value[model_id]
        if (progress >= entry.progress) {
          entry.progress = progress
          if (downloaded !== undefined) entry.downloaded = downloaded
          if (total_size !== undefined) entry.totalSize = total_size
          if (speed !== undefined) entry.speed = speed
        }
      }
    })

    listen('permission-changed', () => {
      fetchPermissions()
    })

    listen('models-changed', () => {
      fetchModels()
    })
  }

  // Initialize
  let initialized = false
  async function init() {
    if (initialized) return
    initialized = true
    setupListeners()
    await Promise.all([
      fetchState(),
      settingsStore.fetchSettings(),
      fetchEngines(),
      fetchModels(),
      fetchLanguages(),
      fetchPermissions(),
      historyStore.fetchHistory(),
      fetchProviders(),
    ])
  }

  return {
    // State
    isRecording, isTranscribing, queueCount,
    activeDownloads, deletingModels,
    selectedModelId, selectedLanguage,
    hallucinationFilterEnabled, appLocale, selectedInputDeviceUid,
    cancelShortcut, recordingMode, hotkey,
    textCleanupEnabled, cleanupModelId, llmModel, llmMaxTokens, asrCloudModel, gpuMode,
    audioDuckingEnabled, audioDuckingLevel,
    engines, models, languages, history,
    audioDevices, permissions, providers,
    // Computed
    isBusy, selectedEngine, downloadedModels, asrEngines, llmEngines, downloadedLlmModels, punctuationEngines, downloadedPunctuationModels, bertModelReady,
    asrModels, isCloudAsr, asrCloudProviderId,
    cleanupModels, isCloudLlm, isLocalLlm, cleanupCloudProviderId,
    // Actions
    init, fetchEngines, fetchModels, fetchLanguages,
    fetchAudioDevices, fetchPermissions, fetchHistory,
    fetchProviders, fetchState,
    selectModel, selectLanguageAction,
    downloadModel, pauseDownload, cancelDownload, deleteModel,
    fetchSettings, setSetting,
    clearHistoryAction, searchHistory, deleteHistoryEntry, deleteHistoryDay,
    requestPermission,
    startMonitoring,
    addProvider, removeProvider, updateProvider,
  }
})
