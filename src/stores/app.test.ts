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
    return () => { delete listeners[event] }
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

  describe('event listeners', () => {
    function setupStore() {
      // Mock all the init invokes to avoid errors
      mockInvoke.mockResolvedValue({ is_recording: false, is_transcribing: false, queue_count: 0, active_downloads: {} })
      const store = useAppStore()
      // Trigger setupListeners by calling init (but we need the listeners registered)
      // setupListeners is called inside init, so we just need the events registered
      // Let's manually trigger by calling init
      store.init()
      return store
    }

    it('recording-started sets isRecording true', async () => {
      const store = setupStore()
      // Wait for init promises
      await vi.waitFor(() => expect(listeners['recording-started']).toBeDefined())

      listeners['recording-started']!({ payload: {} })
      expect(store.isRecording).toBe(true)
    })

    it('recording-stopped sets isRecording false and updates queueCount', async () => {
      const store = setupStore()
      await vi.waitFor(() => expect(listeners['recording-stopped']).toBeDefined())

      store.isRecording = true
      listeners['recording-stopped']!({ payload: { queue_count: 3 } })
      expect(store.isRecording).toBe(false)
      expect(store.queueCount).toBe(3)
    })

    it('transcription-cancelled resets isTranscribing and zeroes queueCount', async () => {
      const store = setupStore()
      await vi.waitFor(() => expect(listeners['transcription-cancelled']).toBeDefined())

      store.isTranscribing = true
      store.queueCount = 5
      listeners['transcription-cancelled']!({ payload: {} })
      expect(store.isTranscribing).toBe(false)
      expect(store.queueCount).toBe(0)
    })
  })
})
