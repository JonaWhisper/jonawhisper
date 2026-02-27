import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'

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
    engines.value = await invoke('get_engines')
  }

  async function fetchModels() {
    models.value = await invoke('get_models')
  }

  async function fetchLanguages() {
    languages.value = await invoke('get_languages')
  }

  async function fetchAudioDevices() {
    audioDevices.value = await invoke('get_audio_devices')
  }

  async function fetchPermissions() {
    permissions.value = await invoke('get_permissions')
  }

  async function fetchHistory() {
    history.value = await invoke('get_history')
  }

  async function fetchApiServers() {
    apiServers.value = await invoke('get_api_servers')
  }

  async function fetchState() {
    const state: any = await invoke('get_app_state')
    isRecording.value = state.is_recording
    isTranscribing.value = state.is_transcribing
    queueCount.value = state.queue_count
    downloadingModelId.value = state.downloading_model_id
    downloadProgress.value = state.download_progress
    selectedModelId.value = state.selected_model_id
    selectedLanguage.value = state.selected_language
    postProcessingEnabled.value = state.post_processing_enabled
    hotkey.value = state.hotkey
  }

  async function selectModel(id: string) {
    await invoke('select_model', { id })
    selectedModelId.value = id
  }

  async function selectLanguageAction(code: string) {
    await invoke('select_language', { code })
    selectedLanguage.value = code
  }

  async function downloadModel(id: string) {
    downloadingModelId.value = id
    downloadProgress.value = 0
    const success = await invoke('download_model_cmd', { id })
    downloadingModelId.value = null
    downloadProgress.value = 0
    if (success) {
      await fetchModels()
    }
    return success
  }

  async function deleteModel(id: string) {
    const success = await invoke('delete_model_cmd', { id })
    if (success) {
      await fetchModels()
    }
    return success
  }

  async function setPostProcessing(enabled: boolean) {
    await invoke('set_post_processing_enabled', { enabled })
    postProcessingEnabled.value = enabled
  }

  async function setHotkey(key: string) {
    await invoke('set_hotkey', { hotkey: key })
    hotkey.value = key
  }

  async function startMonitoring() {
    await invoke('start_monitoring')
  }

  async function clearHistoryAction() {
    await invoke('clear_history')
    history.value = []
  }

  async function requestPermission(kind: string) {
    await invoke('request_permission', { kind })
    // Poll permissions after a short delay
    setTimeout(fetchPermissions, 1500)
  }

  async function addApiServer(config: ApiServerConfig) {
    await invoke('add_api_server', { config })
    await fetchApiServers()
    await fetchEngines()
    await fetchModels()
  }

  async function removeApiServer(id: string) {
    await invoke('remove_api_server', { id })
    await fetchApiServers()
    await fetchEngines()
    await fetchModels()
  }

  // Event listeners
  function setupListeners() {
    listen('recording-started', () => {
      isRecording.value = true
    })

    listen('recording-stopped', (event: any) => {
      isRecording.value = false
      if (event.payload?.queue_count !== undefined) {
        queueCount.value = event.payload.queue_count
      }
    })

    listen('spectrum-data', (event: any) => {
      spectrumData.value = event.payload as number[]
    })

    listen('transcription-started', (event: any) => {
      isTranscribing.value = true
      if (event.payload?.queue_count !== undefined) {
        queueCount.value = event.payload.queue_count
      }
    })

    listen('transcription-complete', (event: any) => {
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

    listen('download-progress', (event: any) => {
      if (event.payload?.progress !== undefined) {
        downloadProgress.value = event.payload.progress
      }
      if (event.payload?.model_id) {
        downloadingModelId.value = event.payload.model_id
      }
    })

    listen('download-complete', (_event: any) => {
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
    postProcessingEnabled, hotkey, spectrumData,
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
