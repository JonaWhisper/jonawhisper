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
  install_hint: string
  available: boolean
  tool_name: string | null
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
  llm_enabled: boolean
  llm_provider_id: string
  llm_model: string
  asr_provider_id: string
  asr_cloud_model: string
  gpu_mode: string
}

export const useAppStore = defineStore('app', () => {
  // State
  const isRecording = ref(false)
  const isTranscribing = ref(false)
  const queueCount = ref(0)
  const activeDownloads = ref<Record<string, { progress: number; stopping: boolean; downloaded: number; totalSize: number; speed: number }>>({})
  const deletingModels = ref<Record<string, boolean>>({})
  const selectedModelId = ref('whisper:large-v3-turbo')
  const selectedLanguage = ref('auto')
  const postProcessingEnabled = ref(true)
  const hallucinationFilterEnabled = ref(true)
  const appLocale = ref('auto')
  const cancelShortcut = ref('escape')
  const recordingMode = ref('push_to_talk')
  const selectedInputDeviceUid = ref<string | null>(null)
  const hotkey = ref('right_command')
  const llmEnabled = ref(false)
  const llmProviderId = ref('')
  const llmModel = ref('')
  const asrProviderId = ref('')
  const asrCloudModel = ref('whisper-1')
  const gpuMode = ref('auto')
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
      await fetchModels()
      const { [id]: _, ...rest } = activeDownloads.value
      activeDownloads.value = rest
    }
  }

  async function pauseDownload(id: string) {
    if (activeDownloads.value[id]) {
      activeDownloads.value = { ...activeDownloads.value, [id]: { ...activeDownloads.value[id], stopping: true } }
    }
    try { await invoke('pause_download', { id }) }
    catch (e) { console.error('pauseDownload failed:', e) }
  }

  async function cancelDownload(id: string) {
    if (activeDownloads.value[id]) {
      activeDownloads.value = { ...activeDownloads.value, [id]: { ...activeDownloads.value[id], stopping: true } }
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
      llmEnabled.value = s.llm_enabled ?? false
      llmProviderId.value = s.llm_provider_id ?? ''
      llmModel.value = s.llm_model ?? ''
      asrProviderId.value = s.asr_provider_id ?? ''
      asrCloudModel.value = s.asr_cloud_model ?? 'whisper-1'
      gpuMode.value = s.gpu_mode ?? 'auto'
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
      case 'llm_enabled': return String(llmEnabled.value)
      case 'llm_provider_id': return llmProviderId.value
      case 'llm_model': return llmModel.value
      case 'asr_provider_id': return asrProviderId.value
      case 'asr_cloud_model': return asrCloudModel.value
      case 'gpu_mode': return gpuMode.value
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
      case 'llm_enabled': llmEnabled.value = value === 'true'; break
      case 'llm_provider_id': llmProviderId.value = value; break
      case 'llm_model': llmModel.value = value; break
      case 'asr_provider_id': asrProviderId.value = value; break
      case 'asr_cloud_model': asrCloudModel.value = value; break
      case 'gpu_mode': gpuMode.value = value; break
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
    llmEnabled, llmProviderId, llmModel, asrProviderId, asrCloudModel, gpuMode,
    engines, models, languages, history,
    audioDevices, permissions, providers,
    // Computed
    isBusy, selectedEngine, downloadedModels,
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
