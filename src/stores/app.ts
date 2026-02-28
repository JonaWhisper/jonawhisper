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
  size: string
  storage_dir: string
  download_type: { type: string }
  download_marker: string | null
  is_downloaded?: boolean
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

export interface ApiServerConfig {
  id: string
  name: string
  url: string
  api_key: string
  model: string
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
}

interface AppStatePayload {
  is_recording: boolean
  is_transcribing: boolean
  queue_count: number
  downloading_model_id: string | null
  download_progress: number
}

export interface LlmConfig {
  enabled: boolean
  provider: string
  api_url: string
  api_key: string
  model: string
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
  llm_config: LlmConfig
}

export const useAppStore = defineStore('app', () => {
  // State
  const isRecording = ref(false)
  const isTranscribing = ref(false)
  const queueCount = ref(0)
  const downloadingModelId = ref<string | null>(null)
  const downloadProgress = ref(0)
  const selectedModelId = ref('whisper:large-v3-turbo')
  const selectedLanguage = ref('auto')
  const postProcessingEnabled = ref(true)
  const hallucinationFilterEnabled = ref(true)
  const appLocale = ref('auto')
  const cancelShortcut = ref('escape')
  const recordingMode = ref('push_to_talk')
  const selectedInputDeviceUid = ref<string | null>(null)
  const hotkey = ref('right_command')
  const llmConfig = ref<LlmConfig>({ enabled: false, provider: 'openai', api_url: '', api_key: '', model: '' })
  const spectrumData = ref<number[]>(new Array(12).fill(0))
  const pillMode = ref<'recording' | 'transcribing' | 'downloading' | 'error' | 'idle'>('recording')

  const engines = ref<EngineInfo[]>([])
  const models = ref<ASRModel[]>([])
  const languages = ref<Language[]>([])
  const history = ref<HistoryEntry[]>([])
  const audioDevices = ref<AudioDevice[]>([])
  const permissions = ref<PermissionReport>({ microphone: 'Undetermined', accessibility: 'Denied', input_monitoring: 'Denied' })
  const apiServers = ref<ApiServerConfig[]>([])

  // Computed
  const isBusy = computed(() => isRecording.value || isTranscribing.value || queueCount.value > 0 || downloadingModelId.value !== null)
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

  async function fetchApiServers() {
    try { apiServers.value = await invoke('get_api_servers') }
    catch (e) { console.error('fetchApiServers failed:', e) }
  }

  async function fetchState() {
    try {
      const state = await invoke<AppStatePayload>('get_app_state')
      isRecording.value = state.is_recording
      isTranscribing.value = state.is_transcribing
      queueCount.value = state.queue_count
      downloadingModelId.value = state.downloading_model_id
      downloadProgress.value = state.download_progress
    } catch (e) { console.error('fetchState failed:', e) }
  }

  async function selectModel(id: string) {
    try {
      await invoke('select_model', { id })
      selectedModelId.value = id
    } catch (e) { console.error('selectModel failed:', e) }
  }

  async function selectLanguageAction(code: string) {
    try {
      await invoke('select_language', { code })
      selectedLanguage.value = code
    } catch (e) { console.error('selectLanguageAction failed:', e) }
  }

  async function downloadModel(id: string) {
    downloadingModelId.value = id
    downloadProgress.value = 0
    try {
      const success = await invoke<boolean>('download_model_cmd', { id })
      if (success) {
        await fetchModels()
      }
      return success
    } catch (e) {
      console.error('downloadModel failed:', e)
      return false
    } finally {
      downloadingModelId.value = null
      downloadProgress.value = 0
    }
  }

  async function deleteModel(id: string) {
    try {
      const success = await invoke<boolean>('delete_model_cmd', { id })
      if (success) {
        await fetchModels()
      }
      return success
    } catch (e) {
      console.error('deleteModel failed:', e)
      return false
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
      if (s.llm_config) llmConfig.value = s.llm_config
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

  async function setLlmConfig(config: LlmConfig) {
    const prev = { ...llmConfig.value }
    llmConfig.value = config
    try {
      await invoke('set_llm_config', { config })
    } catch (e) {
      console.error('setLlmConfig failed, rolling back:', e)
      llmConfig.value = prev
    }
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

  async function addApiServer(config: ApiServerConfig) {
    try {
      await invoke('add_api_server', { config })
      await fetchApiServers()
      await fetchEngines()
      await fetchModels()
    } catch (e) { console.error('addApiServer failed:', e) }
  }

  async function removeApiServer(id: string) {
    try {
      await invoke('remove_api_server', { id })
      await fetchApiServers()
      await fetchEngines()
      await fetchModels()
    } catch (e) { console.error('removeApiServer failed:', e) }
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
      if (event.payload?.progress !== undefined) {
        downloadProgress.value = event.payload.progress
      }
      if (event.payload?.model_id) {
        downloadingModelId.value = event.payload.model_id
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
      fetchApiServers(),
    ])
  }

  return {
    // State
    isRecording, isTranscribing, queueCount,
    downloadingModelId, downloadProgress,
    selectedModelId, selectedLanguage,
    postProcessingEnabled, hallucinationFilterEnabled, appLocale, selectedInputDeviceUid,
    cancelShortcut, recordingMode, hotkey, spectrumData, pillMode,
    llmConfig,
    engines, models, languages, history,
    audioDevices, permissions, apiServers,
    // Computed
    isBusy, selectedEngine, downloadedModels,
    // Actions
    init, fetchEngines, fetchModels, fetchLanguages,
    fetchAudioDevices, fetchPermissions, fetchHistory,
    fetchApiServers, fetchState,
    selectModel, selectLanguageAction,
    downloadModel, deleteModel,
    fetchSettings, setSetting, setLlmConfig,
    clearHistoryAction, searchHistory, deleteHistoryEntry, deleteHistoryDay,
    requestPermission,
    startMonitoring,
    addApiServer, removeApiServer,
  }
})
