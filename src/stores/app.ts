import { defineStore } from 'pinia'
import { ref, computed } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type Event } from '@tauri-apps/api/event'
import { useSettingsStore } from './settings'
import { useHistoryStore } from './history'
import { useEnginesStore } from './engines'
import { useDownloadStore } from './downloads'

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

  // Runtime state
  const isRecording = ref(false)
  const isTranscribing = ref(false)
  const queueCount = ref(0)

  const isBusy = computed(() =>
    isRecording.value || isTranscribing.value || queueCount.value > 0
    || Object.keys(downloadStore.activeDownloads).length > 0
  )

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

  async function startMonitoring() {
    try { await invoke('start_monitoring') }
    catch (e) { console.error('startMonitoring failed:', e) }
  }

  // Event listeners (cross-domain: recording + transcription)
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
          vad_trimmed: event.payload.vad_trimmed ?? false,
          punctuation_model_id: event.payload.punctuation_model_id ?? '',
          spellcheck: event.payload.spellcheck ?? false,
          disfluency_removal: event.payload.disfluency_removal ?? false,
          itn: event.payload.itn ?? false,
          raw_text: event.payload.raw_text ?? '',
          word_scores: event.payload.word_scores ?? '',
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

  // Initialize all stores
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
      enginesStore.fetchProviders(),
    ])
  }

  return {
    isRecording, isTranscribing, queueCount,
    isBusy,
    init, fetchState, startMonitoring,
  }
})
