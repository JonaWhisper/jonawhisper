import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type Event } from '@tauri-apps/api/event'

export interface AudioDevice {
  id: number
  name: string
  uid: string
  transport_type: string
  is_default: boolean
}

export interface EngineInfo {
  id: string
  name: string
  description: string
  category: 'asr' | 'llm' | 'punctuation'
  available: boolean
  supported_language_codes: string[]
}

export interface ASRModel {
  id: string
  engine_id: string
  label: string
  filename: string
  url: string
  size: number
  storage_dir: string
  download_type: { type: string }
  download_marker: string | null
  is_downloaded?: boolean
  recommended?: boolean
  partial_progress: number | null
  wer: number | null
  rtf: number | null
  params: number | null
  ram: number | null
  lang_codes: string[] | null
}

export interface Language {
  code: string
  label: string
}

export interface HistoryEntry {
  text: string
  timestamp: number
  model_id: string
  language: string
}

export type ProviderKind = 'OpenAI' | 'Anthropic' | 'Custom'

export interface Provider {
  id: string
  name: string
  kind: ProviderKind
  url: string
  api_key: string
}

export interface PermissionReport {
  microphone: string
  accessibility: string
  input_monitoring: string
}

// Tauri event payload types
interface RecordingStoppedPayload {
  queue_count?: number
}

interface TranscriptionStartedPayload {
  queue_count?: number
}

interface TranscriptionCompletePayload {
  text?: string
}

interface DownloadProgressPayload {
  model_id?: string
  progress?: number
  downloaded?: number
  total_size?: number
  speed?: number
}

interface AppStatePayload {
  is_recording: boolean
  is_transcribing: boolean
  queue_count: number
  active_downloads: Record<string, number>
}

export interface SettingsPayload {
  app_locale: string
  post_processing_enabled: boolean
  hallucination_filter_enabled: boolean
  hotkey: string
  cancel_shortcut: string
  recording_mode: string
  selected_input_device_uid: string | null
  selected_model_id: string
  selected_language: string
  text_cleanup_enabled: boolean
  cleanup_model_id: string
  llm_provider_id: string
  llm_model: string
  asr_provider_id: string
  asr_cloud_model: string
  gpu_mode: string
  llm_max_tokens: number
}

export const useAppStore = defineStore('app', () => {
  // State
  const isRecording = ref(false)
  const isTranscribing = ref(false)
  const queueCount = ref(0)
  const activeDownloads = ref<Record<string, { progress: number; stopping: boolean; downloaded: number; totalSize: number; speed: number }>>({})
  const deletingModels = ref<Record<string, boolean>>({})
  const selectedModelId = ref('whisper:large-v3-turbo-q8')
  const selectedLanguage = ref('auto')
  const postProcessingEnabled = ref(true)
  const hallucinationFilterEnabled = ref(true)
  const appLocale = ref('auto')
  const cancelShortcut = ref('escape')
  const recordingMode = ref('push_to_talk')
  const selectedInputDeviceUid = ref<string | null>(null)
  const hotkey = ref('right_command')
  const textCleanupEnabled = ref(false)
  const cleanupModelId = ref('')
  const llmProviderId = ref('')
  const llmModel = ref('')
  const asrProviderId = ref('')
  const asrCloudModel = ref('whisper-1')
  const gpuMode = ref('auto')
  const llmMaxTokens = ref(256)
  const spectrumData = ref<number[]>(new Array(12).fill(0))
  const pillMode = ref<'recording' | 'transcribing' | 'error' | 'idle'>('recording')

  const engines = ref<EngineInfo[]>([])
  const models = ref<ASRModel[]>([])
  const languages = ref<Language[]>([])
  const history = ref<HistoryEntry[]>([])
  const audioDevices = ref<AudioDevice[]>([])
  const permissions = ref<PermissionReport>({ microphone: 'Undetermined', accessibility: 'Denied', input_monitoring: 'Denied' })
  const providers = ref<Provider[]>([])

  // Computed
  const isBusy = computed(() => isRecording.value || isTranscribing.value || queueCount.value > 0 || Object.keys(activeDownloads.value).length > 0)
  const selectedEngine = computed(() => {
    const model = models.value.find(m => m.id === selectedModelId.value)
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
  const isCloudLlm = computed(() => cleanupModelId.value.startsWith('cloud:'))
  const isLocalLlm = computed(() => cleanupModelId.value.startsWith('llama:'))
  const cleanupCloudProviderId = computed(() =>
    isCloudLlm.value ? cleanupModelId.value.slice('cloud:'.length) : ''
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

  async function fetchHistory() {
    try { history.value = await invoke('get_history') }
    catch (e) { console.error('fetchHistory failed:', e) }
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

  async function selectModel(id: string) {
    await setSetting('selected_model_id', id)
  }

  async function selectLanguageAction(code: string) {
    await setSetting('selected_language', code)
  }

  async function downloadModel(id: string) {
    if (activeDownloads.value[id]) return false
    // Pre-fill progress from partial file (avoids 0% flash on resume)
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
      // If pauseDownload already cleaned up, skip
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
    // Optimistic: immediately show paused state (no intermediate spinner)
    const m = models.value.find(m => m.id === id)
    if (m && entry.progress > 0) {
      m.partial_progress = entry.progress
    }
    const { [id]: _, ...rest } = activeDownloads.value
    activeDownloads.value = rest
    // Tell backend to stop (fire-and-forget)
    try { await invoke('pause_download', { id }) }
    catch (e) { console.error('pauseDownload failed:', e) }
  }

  async function cancelDownload(id: string) {
    if (activeDownloads.value[id]) {
      activeDownloads.value = { ...activeDownloads.value, [id]: { ...activeDownloads.value[id], stopping: true } }
    } else {
      // Cancelling from paused state — clear partial_progress immediately
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



  async function fetchSettings() {
    try {
      const s = await invoke<SettingsPayload>('get_settings')
      appLocale.value = s.app_locale
      postProcessingEnabled.value = s.post_processing_enabled
      hallucinationFilterEnabled.value = s.hallucination_filter_enabled
      hotkey.value = s.hotkey
      selectedInputDeviceUid.value = s.selected_input_device_uid
      cancelShortcut.value = s.cancel_shortcut
      recordingMode.value = s.recording_mode
      selectedModelId.value = s.selected_model_id
      selectedLanguage.value = s.selected_language
      textCleanupEnabled.value = s.text_cleanup_enabled ?? false
      cleanupModelId.value = s.cleanup_model_id ?? ''
      llmProviderId.value = s.llm_provider_id ?? ''
      llmModel.value = s.llm_model ?? ''
      asrProviderId.value = s.asr_provider_id ?? ''
      asrCloudModel.value = s.asr_cloud_model ?? 'whisper-1'
      gpuMode.value = s.gpu_mode ?? 'auto'
      llmMaxTokens.value = s.llm_max_tokens ?? 256
    } catch (e) { console.error('fetchSettings failed:', e) }
  }

  function getSettingValue(key: string): string {
    switch (key) {
      case 'app_locale': return appLocale.value
      case 'post_processing_enabled': return String(postProcessingEnabled.value)
      case 'hallucination_filter_enabled': return String(hallucinationFilterEnabled.value)
      case 'hotkey': return hotkey.value
      case 'cancel_shortcut': return cancelShortcut.value
      case 'recording_mode': return recordingMode.value
      case 'selected_input_device_uid': return selectedInputDeviceUid.value ?? ''
      case 'selected_model_id': return selectedModelId.value
      case 'selected_language': return selectedLanguage.value
      case 'text_cleanup_enabled': return String(textCleanupEnabled.value)
      case 'cleanup_model_id': return cleanupModelId.value
      case 'llm_provider_id': return llmProviderId.value
      case 'llm_model': return llmModel.value
      case 'asr_provider_id': return asrProviderId.value
      case 'asr_cloud_model': return asrCloudModel.value
      case 'gpu_mode': return gpuMode.value
      case 'llm_max_tokens': return String(llmMaxTokens.value)
      default: return ''
    }
  }

  function applySettingLocally(key: string, value: string) {
    switch (key) {
      case 'app_locale': appLocale.value = value; break
      case 'post_processing_enabled': postProcessingEnabled.value = value === 'true'; break
      case 'hallucination_filter_enabled': hallucinationFilterEnabled.value = value === 'true'; break
      case 'hotkey': hotkey.value = value; break
      case 'cancel_shortcut': cancelShortcut.value = value; break
      case 'recording_mode': recordingMode.value = value; break
      case 'selected_input_device_uid': selectedInputDeviceUid.value = value || null; break
      case 'selected_model_id': selectedModelId.value = value; break
      case 'selected_language': selectedLanguage.value = value; break
      case 'text_cleanup_enabled': textCleanupEnabled.value = value === 'true'; break
      case 'cleanup_model_id': cleanupModelId.value = value; break
      case 'llm_provider_id': llmProviderId.value = value; break
      case 'llm_model': llmModel.value = value; break
      case 'asr_provider_id': asrProviderId.value = value; break
      case 'asr_cloud_model': asrCloudModel.value = value; break
      case 'gpu_mode': gpuMode.value = value; break
      case 'llm_max_tokens': llmMaxTokens.value = parseInt(value, 10) || 256; break
    }
  }

  async function setSetting(key: string, value: string) {
    const prev = getSettingValue(key)
    applySettingLocally(key, value)
    try {
      await invoke('set_setting', { key, value })
    } catch (e) {
      console.error('setSetting failed, rolling back:', e)
      applySettingLocally(key, prev)
    }
  }

  async function startMonitoring() {
    try { await invoke('start_monitoring') }
    catch (e) { console.error('startMonitoring failed:', e) }
  }


  async function clearHistoryAction() {
    try {
      await invoke('clear_history')
      history.value = []
    } catch (e) { console.error('clearHistory failed:', e) }
  }

  async function searchHistory(query: string): Promise<HistoryEntry[]> {
    try {
      return await invoke<HistoryEntry[]>('search_history', { query })
    } catch (e) {
      console.error('searchHistory failed:', e)
      return []
    }
  }

  async function deleteHistoryEntry(timestamp: number) {
    try {
      await invoke('delete_history_entry', { timestamp: Math.floor(timestamp) })
      history.value = history.value.filter(e => e.timestamp !== timestamp)
    } catch (e) { console.error('deleteHistoryEntry failed:', e) }
  }

  async function deleteHistoryDay(dayTimestamp: number) {
    try {
      await invoke('delete_history_day', { dayTimestamp: Math.floor(dayTimestamp) })
      const dayEnd = dayTimestamp + 86400
      history.value = history.value.filter(e => e.timestamp < dayTimestamp || e.timestamp >= dayEnd)
    } catch (e) { console.error('deleteHistoryDay failed:', e) }
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

  // Event listeners (store is a singleton — listeners live for the app's lifetime)
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

    listen<number[]>('spectrum-data', (event: Event<number[]>) => {
      spectrumData.value = event.payload
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
        history.value.unshift({
          text: event.payload.text,
          timestamp: Date.now() / 1000,
          model_id: selectedModelId.value,
          language: selectedLanguage.value,
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

    listen<string>('pill-mode', (event: Event<string>) => {
      pillMode.value = event.payload as typeof pillMode.value
    })

    listen<DownloadProgressPayload>('download-progress', (event: Event<DownloadProgressPayload>) => {
      const { model_id, progress, downloaded, total_size, speed } = event.payload ?? {}
      if (model_id && progress !== undefined && activeDownloads.value[model_id]) {
        const entry = activeDownloads.value[model_id]
        // Only accept forward progress (avoid jitter from out-of-order events)
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

  // Initialize (don't fetch audio devices here — that triggers mic permission dialog on macOS 14+)
  let initialized = false
  async function init() {
    if (initialized) return
    initialized = true
    setupListeners()
    await Promise.all([
      fetchState(),
      fetchSettings(),
      fetchEngines(),
      fetchModels(),
      fetchLanguages(),
      fetchPermissions(),
      fetchHistory(),
      fetchProviders(),
    ])
  }

  return {
    // State
    isRecording, isTranscribing, queueCount,
    activeDownloads, deletingModels,
    selectedModelId, selectedLanguage,
    postProcessingEnabled, hallucinationFilterEnabled, appLocale, selectedInputDeviceUid,
    cancelShortcut, recordingMode, hotkey, spectrumData, pillMode,
    textCleanupEnabled, cleanupModelId, llmProviderId, llmModel, llmMaxTokens, asrProviderId, asrCloudModel, gpuMode,
    engines, models, languages, history,
    audioDevices, permissions, providers,
    // Computed
    isBusy, selectedEngine, downloadedModels, asrEngines, llmEngines, downloadedLlmModels, punctuationEngines, downloadedPunctuationModels, bertModelReady, cleanupModels, isCloudLlm, isLocalLlm, cleanupCloudProviderId,
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
