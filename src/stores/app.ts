import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type Event } from '@tauri-apps/api/event'
import { useSettingsStore } from './settings'
import { useHistoryStore } from './history'
import { useEnginesStore } from './engines'
import { useDownloadStore } from './downloads'

// Re-export all types for backward compatibility
export type {
  AudioDevice, EngineInfo, ASRModel, Language, HistoryEntry,
  ProviderKind, Provider, PermissionReport, SettingsPayload,
} from './types'

import type {
  RecordingStoppedPayload, TranscriptionStartedPayload,
  TranscriptionCompletePayload,
  AppStatePayload,
} from './types'

export const useAppStore = defineStore('app', () => {
  const settingsStore = useSettingsStore()
  const historyStore = useHistoryStore()
  const enginesStore = useEnginesStore()
  const downloadStore = useDownloadStore()

  // State
  const isRecording = ref(false)
  const isTranscribing = ref(false)
  const queueCount = ref(0)

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

  // Proxy refs for engines backward compat
  const engines = computed(() => enginesStore.engines)
  const models = computed(() => enginesStore.models)
  const languages = computed(() => enginesStore.languages)
  const audioDevices = computed(() => enginesStore.audioDevices)
  const permissions = computed(() => enginesStore.permissions)
  const providers = computed(() => enginesStore.providers)
  const selectedEngine = computed(() => enginesStore.selectedEngine)
  const downloadedModels = computed(() => enginesStore.downloadedModels)
  const asrEngines = computed(() => enginesStore.asrEngines)
  const llmEngines = computed(() => enginesStore.llmEngines)
  const downloadedLlmModels = computed(() => enginesStore.downloadedLlmModels)
  const punctuationEngines = computed(() => enginesStore.punctuationEngines)
  const downloadedPunctuationModels = computed(() => enginesStore.downloadedPunctuationModels)
  const bertModelReady = computed(() => enginesStore.bertModelReady)
  const cleanupModels = computed(() => enginesStore.cleanupModels)
  const asrModels = computed(() => enginesStore.asrModels)
  const isCloudAsr = computed(() => enginesStore.isCloudAsr)
  const asrCloudProviderId = computed(() => enginesStore.asrCloudProviderId)
  const isCloudLlm = computed(() => enginesStore.isCloudLlm)
  const isLocalLlm = computed(() => enginesStore.isLocalLlm)
  const cleanupCloudProviderId = computed(() => enginesStore.cleanupCloudProviderId)

  // Proxy refs for downloads backward compat
  const activeDownloads = computed(() => downloadStore.activeDownloads)
  const deletingModels = computed(() => downloadStore.deletingModels)

  // Computed
  const isBusy = computed(() => isRecording.value || isTranscribing.value || queueCount.value > 0 || Object.keys(downloadStore.activeDownloads).length > 0)

  // Actions
  async function fetchState() {
    try {
      const state = await invoke<AppStatePayload>('get_app_state')
      isRecording.value = state.is_recording
      isTranscribing.value = state.is_transcribing
      queueCount.value = state.queue_count
      downloadStore.hydrateFromBackend(state.active_downloads)
    } catch (e) { console.error('fetchState failed:', e) }
  }

  // Delegated settings actions
  const { fetchSettings, setSetting, selectModel, selectLanguageAction } = settingsStore

  // Delegated engines actions
  const { fetchEngines, fetchModels, fetchLanguages, fetchAudioDevices, fetchPermissions, fetchProviders, requestPermission, addProvider, removeProvider, updateProvider } = enginesStore

  // Delegated download actions
  const { downloadModel, pauseDownload, cancelDownload, deleteModel } = downloadStore

  async function startMonitoring() {
    try { await invoke('start_monitoring') }
    catch (e) { console.error('startMonitoring failed:', e) }
  }

  // Delegated history actions
  const { fetchHistory, clearHistoryAction, searchHistory, deleteHistoryEntry, deleteHistoryDay } = historyStore

  // Event listeners
  function setupListeners() {
    enginesStore.setupListeners()
    downloadStore.setupListeners()

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
      enginesStore.fetchEngines(),
      enginesStore.fetchModels(),
      enginesStore.fetchLanguages(),
      enginesStore.fetchPermissions(),
      historyStore.fetchHistory(),
      enginesStore.fetchProviders(),
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
