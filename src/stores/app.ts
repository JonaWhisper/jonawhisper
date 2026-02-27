import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type Event } from '@tauri-apps/api/event'

export interface AudioDevice {
  name: string
  uid: string
  is_default: boolean
}

export interface EngineInfo {
  id: string
  name: string
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
}

export interface Language {
  code: string
  label: string
}

export interface HistoryEntry {
  text: string
  timestamp: number
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

interface DownloadCompletePayload {
  model_id: string
  success: boolean
}

interface AppStatePayload {
  is_recording: boolean
  is_transcribing: boolean
  queue_count: number
  downloading_model_id: string | null
  download_progress: number
  selected_model_id: string
  selected_language: string
  post_processing_enabled: boolean
  hotkey: string
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
  const hotkey = ref('right_command')
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
      selectedModelId.value = state.selected_model_id
      selectedLanguage.value = state.selected_language
      postProcessingEnabled.value = state.post_processing_enabled
      hotkey.value = state.hotkey
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

  async function setPostProcessing(enabled: boolean) {
    try {
      await invoke('set_post_processing_enabled', { enabled })
      postProcessingEnabled.value = enabled
    } catch (e) { console.error('setPostProcessing failed:', e) }
  }

  async function setHotkey(key: string) {
    try {
      await invoke('set_hotkey', { hotkey: key })
      hotkey.value = key
    } catch (e) { console.error('setHotkey failed:', e) }
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
        })
        if (history.value.length > 20) {
          history.value = history.value.slice(0, 20)
        }
      }
    })

    listen('transcription-error', () => {
      isTranscribing.value = false
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

    listen<DownloadCompletePayload>('download-complete', () => {
      downloadingModelId.value = null
      downloadProgress.value = 0
      fetchModels()
    })

    listen('permission-changed', () => {
      fetchPermissions()
    })
  }

  // Initialize (don't fetch audio devices here — that triggers mic permission dialog on macOS 14+)
  async function init() {
    setupListeners()
    await Promise.all([
      fetchState(),
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
    postProcessingEnabled, hotkey, spectrumData, pillMode,
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
    setPostProcessing, setHotkey,
    clearHistoryAction, requestPermission,
    startMonitoring,
    addApiServer, removeApiServer,
    setupListeners,
  }
})
