import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const mockInvoke = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}))

const listeners: Record<string, (event: unknown) => void> = {}
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, handler: (event: unknown) => void) => {
    listeners[event] = handler
    return Promise.resolve(() => { delete listeners[event] })
  }),
}))

import { useAppStore } from './app'
import { useDownloadStore } from './downloads'

describe('app store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockInvoke.mockReset()
    Object.keys(listeners).forEach(k => delete listeners[k])
  })

  describe('isBusy', () => {
    it('is true when isRecording', () => {
      const store = useAppStore()
      store.isRecording = true
      expect(store.isBusy).toBe(true)
    })

    it('is true when isTranscribing', () => {
      const store = useAppStore()
      store.isTranscribing = true
      expect(store.isBusy).toBe(true)
    })

    it('is true when queueCount > 0', () => {
      const store = useAppStore()
      store.queueCount = 2
      expect(store.isBusy).toBe(true)
    })

    it('is true when activeDownloads is non-empty', () => {
      const store = useAppStore()
      const downloads = useDownloadStore()
      downloads.activeDownloads = { 'model-a': { progress: 0.5, stopping: false, downloaded: 0, totalSize: 0, speed: 0 } }
      expect(store.isBusy).toBe(true)
    })

    it('is false when everything is idle', () => {
      const store = useAppStore()
      expect(store.isBusy).toBe(false)
    })
  })

  function mockAllInvokes() {
    mockInvoke.mockImplementation(async (cmd: string) => {
      switch (cmd) {
        case 'get_app_state': return { is_recording: false, is_transcribing: false, queue_count: 0, active_downloads: {} }
        case 'get_settings': return { app_locale: 'auto', hallucination_filter_enabled: true, hotkey: 'right_command', cancel_shortcut: 'escape', recording_mode: 'push_to_talk', selected_input_device_uid: null, selected_model_id: '', selected_language: 'auto', text_cleanup_enabled: false, cleanup_model_id: '', llm_provider_id: '', llm_model: '', asr_cloud_model: 'whisper-1', gpu_mode: 'auto', llm_max_tokens: 4096, audio_ducking_enabled: false, audio_ducking_level: 0.2, vad_enabled: true, punctuation_model_id: '', disfluency_removal_enabled: true, itn_enabled: true, spellcheck_enabled: false, theme: 'system', log_level: 'info', log_retention: 'previous' }
        case 'get_engines': return []
        case 'get_models': return []
        case 'get_providers': return []
        case 'get_provider_presets': return []
        case 'check_for_update': return null
        default: return undefined
      }
    })
  }

  describe('checkForUpdate', () => {
    it('sets updateAvailable when update found', async () => {
      const store = useAppStore()
      const update = { version: '2.0.0', body: 'New features' }
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'check_for_update') return update
        return undefined
      })

      await store.checkForUpdate()

      expect(store.updateAvailable).toEqual(update)
      expect(store.updateChecking).toBe(false)
      expect(store.updateError).toBe('')
    })

    it('sets updateAvailable to null when no update', async () => {
      const store = useAppStore()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'check_for_update') return null
        return undefined
      })

      await store.checkForUpdate()

      expect(store.updateAvailable).toBeNull()
      expect(store.updateChecking).toBe(false)
      expect(store.updateError).toBe('')
    })

    it('sets updateError on failure', async () => {
      const store = useAppStore()
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'check_for_update') throw new Error('Network error')
        return undefined
      })

      await store.checkForUpdate()

      expect(store.updateAvailable).toBeNull()
      expect(store.updateChecking).toBe(false)
      expect(store.updateError).toBe('Error: Network error')
    })

    it('sets updateChecking=true during check', async () => {
      const store = useAppStore()
      let resolve: (v: unknown) => void
      mockInvoke.mockImplementation((cmd: string) => {
        if (cmd === 'check_for_update') {
          return new Promise(r => { resolve = r })
        }
        return Promise.resolve(undefined)
      })

      const promise = store.checkForUpdate()
      expect(store.updateChecking).toBe(true)

      resolve!(null)
      await promise
      expect(store.updateChecking).toBe(false)
    })

    it('clears previous error on new check', async () => {
      const store = useAppStore()
      // First: fail
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'check_for_update') throw new Error('fail')
        return undefined
      })
      await store.checkForUpdate()
      expect(store.updateError).toBe('Error: fail')

      // Second: succeed
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'check_for_update') return { version: '2.0.0', body: null }
        return undefined
      })
      await store.checkForUpdate()
      expect(store.updateError).toBe('')
    })
  })

  describe('event listeners', () => {
    async function setupStore() {
      mockAllInvokes()
      const store = useAppStore()
      await store.init()
      return store
    }

    it('recording-started sets isRecording true', async () => {
      const store = await setupStore()

      listeners['recording-started']!({ payload: {} })
      expect(store.isRecording).toBe(true)
    })

    it('recording-stopped sets isRecording false and updates queueCount', async () => {
      const store = await setupStore()

      store.isRecording = true
      listeners['recording-stopped']!({ payload: { queue_count: 3 } })
      expect(store.isRecording).toBe(false)
      expect(store.queueCount).toBe(3)
    })

    it('transcription-cancelled resets isTranscribing and zeroes queueCount', async () => {
      const store = await setupStore()

      store.isTranscribing = true
      store.queueCount = 5
      listeners['transcription-cancelled']!({ payload: {} })
      expect(store.isTranscribing).toBe(false)
      expect(store.queueCount).toBe(0)
    })
  })
})
