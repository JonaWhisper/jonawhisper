import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

// Track invoke calls
const mockInvoke = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}))

// Capture event listeners so we can simulate events
const listeners: Record<string, (event: unknown) => void> = {}
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, handler: (event: unknown) => void) => {
    listeners[event] = handler
    return Promise.resolve(() => { delete listeners[event] })
  }),
}))

import { useDownloadStore } from './downloads'
import { useEnginesStore } from './engines'
import type { ASRModel } from './types'

function makeModel(overrides: Partial<ASRModel> = {}): ASRModel {
  return {
    id: 'test-model',
    engine_id: 'whisper',
    label: 'Test Model',
    filename: 'test.onnx',
    url: 'https://example.com/test.onnx',
    size: 100_000_000,
    storage_dir: '/tmp/models',
    download_type: { type: 'SingleFile' },
    download_marker: null,
    is_downloaded: false,
    recommended: false,
    partial_progress: null,
    wer: null,
    rtf: null,
    params: null,
    ram: null,
    lang_codes: null,
    quantization: null,
    ...overrides,
  }
}

describe('downloads store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockInvoke.mockReset()
    Object.keys(listeners).forEach(k => delete listeners[k])
  })

  describe('downloadModel', () => {
    it('creates active download entry with initial state', async () => {
      const downloads = useDownloadStore()
      const engines = useEnginesStore()
      engines.models = [makeModel({ id: 'whisper:tiny', size: 50_000_000 })]

      mockInvoke.mockResolvedValueOnce(true) // download_model_cmd
      mockInvoke.mockResolvedValueOnce([]) // fetchModels (get_models)

      const promise = downloads.downloadModel('whisper:tiny')

      // Entry should exist immediately
      expect(downloads.activeDownloads['whisper:tiny']).toBeDefined()
      expect(downloads.activeDownloads['whisper:tiny']!.progress).toBe(0)
      expect(downloads.activeDownloads['whisper:tiny']!.stopping).toBe(false)
      expect(downloads.activeDownloads['whisper:tiny']!.totalSize).toBe(50_000_000)
      expect(downloads.activeDownloads['whisper:tiny']!.speed).toBe(0)

      await promise

      // Entry should be cleaned up after completion
      expect(downloads.activeDownloads['whisper:tiny']).toBeUndefined()
    })

    it('sets is_downloaded when progress reaches 1', async () => {
      const downloads = useDownloadStore()
      const engines = useEnginesStore()
      engines.models = [makeModel({ id: 'whisper:tiny' })]

      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'download_model_cmd') {
          // Simulate progress reaching 100%
          downloads.activeDownloads['whisper:tiny']!.progress = 1.0
          return true
        }
        if (cmd === 'get_models') return engines.models
        if (cmd === 'set_setting') return undefined
        return []
      })

      await downloads.downloadModel('whisper:tiny')

      const model = engines.models.find(m => m.id === 'whisper:tiny')!
      expect(model.is_downloaded).toBe(true)
      expect(model.partial_progress).toBeNull()
    })

    it('saves partial_progress when download is incomplete', async () => {
      const downloads = useDownloadStore()
      const engines = useEnginesStore()
      engines.models = [makeModel({ id: 'whisper:tiny' })]

      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'download_model_cmd') {
          downloads.activeDownloads['whisper:tiny']!.progress = 0.5
          return false
        }
        if (cmd === 'get_models') return engines.models
        if (cmd === 'set_setting') return undefined
        return []
      })

      await downloads.downloadModel('whisper:tiny')

      const model = engines.models.find(m => m.id === 'whisper:tiny')!
      expect(model.is_downloaded).toBeFalsy()
      expect(model.partial_progress).toBe(0.5)
    })

    it('does not start duplicate download', async () => {
      const downloads = useDownloadStore()
      const engines = useEnginesStore()
      engines.models = [makeModel({ id: 'whisper:tiny' })]

      mockInvoke.mockResolvedValue(true)
      const p1 = downloads.downloadModel('whisper:tiny')

      // Second call should return false immediately
      const result = await downloads.downloadModel('whisper:tiny')
      expect(result).toBe(false)

      await p1
    })

    it('resumes from partial_progress', async () => {
      const downloads = useDownloadStore()
      const engines = useEnginesStore()
      engines.models = [makeModel({ id: 'whisper:tiny', partial_progress: 0.6 })]

      mockInvoke.mockResolvedValueOnce(true)
      mockInvoke.mockResolvedValueOnce([])

      const promise = downloads.downloadModel('whisper:tiny')

      // Should start from saved progress
      expect(downloads.activeDownloads['whisper:tiny']!.progress).toBe(0.6)

      await promise
    })
  })

  describe('pauseDownload', () => {
    it('marks entry as stopping before calling backend', async () => {
      const downloads = useDownloadStore()
      const engines = useEnginesStore()
      engines.models = [makeModel({ id: 'whisper:tiny' })]

      let stoppingDuringInvoke = false
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'pause_download') {
          // Check that stopping is true while we wait for backend
          stoppingDuringInvoke = downloads.activeDownloads['whisper:tiny']?.stopping ?? false
        }
      })

      // Manually set up active download
      downloads.activeDownloads = { 'whisper:tiny': { progress: 0.5, stopping: false, downloaded: 50_000_000, totalSize: 100_000_000, speed: 1_000_000 } }

      await downloads.pauseDownload('whisper:tiny')

      expect(stoppingDuringInvoke).toBe(true)
    })

    it('saves progress and removes entry after backend confirms', async () => {
      const downloads = useDownloadStore()
      const engines = useEnginesStore()
      engines.models = [makeModel({ id: 'whisper:tiny' })]

      mockInvoke.mockResolvedValue(undefined)

      downloads.activeDownloads = { 'whisper:tiny': { progress: 0.5, stopping: false, downloaded: 50_000_000, totalSize: 100_000_000, speed: 1_000_000 } }

      await downloads.pauseDownload('whisper:tiny')

      // Entry should be removed
      expect(downloads.activeDownloads['whisper:tiny']).toBeUndefined()

      // Progress should be saved on model
      const model = engines.models.find(m => m.id === 'whisper:tiny')!
      expect(model.partial_progress).toBe(0.5)
    })

    it('does nothing if no active download', async () => {
      const downloads = useDownloadStore()

      await downloads.pauseDownload('nonexistent')

      expect(mockInvoke).not.toHaveBeenCalled()
    })
  })

  describe('cancelDownload', () => {
    it('marks entry as stopping', async () => {
      const downloads = useDownloadStore()
      const engines = useEnginesStore()
      engines.models = [makeModel({ id: 'whisper:tiny' })]

      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_models') return engines.models
        if (cmd === 'set_setting') return undefined
        return undefined
      })

      downloads.activeDownloads = { 'whisper:tiny': { progress: 0.3, stopping: false, downloaded: 30_000_000, totalSize: 100_000_000, speed: 500_000 } }

      const promise = downloads.cancelDownload('whisper:tiny')

      expect(downloads.activeDownloads['whisper:tiny']?.stopping).toBe(true)

      await promise
    })

    it('clears partial_progress when no active download', async () => {
      const downloads = useDownloadStore()
      const engines = useEnginesStore()
      engines.models = [makeModel({ id: 'whisper:tiny', partial_progress: 0.4 })]

      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'get_models') return engines.models
        if (cmd === 'set_setting') return undefined
        return undefined
      })

      await downloads.cancelDownload('whisper:tiny')

      const model = engines.models.find(m => m.id === 'whisper:tiny')!
      expect(model.partial_progress).toBeNull()
    })
  })

  describe('deleteModel', () => {
    it('tracks deleting state', async () => {
      const downloads = useDownloadStore()
      const engines = useEnginesStore()
      engines.models = [makeModel({ id: 'whisper:tiny', is_downloaded: true })]

      let deletingDuringInvoke = false
      mockInvoke.mockImplementation(async (cmd: string) => {
        if (cmd === 'delete_model_cmd') {
          deletingDuringInvoke = !!downloads.deletingModels['whisper:tiny']
          return true
        }
        return []
      })

      await downloads.deleteModel('whisper:tiny')

      expect(deletingDuringInvoke).toBe(true)
      // Should be cleared after completion
      expect(downloads.deletingModels['whisper:tiny']).toBeUndefined()
    })
  })

  describe('hydrateFromBackend', () => {
    it('populates activeDownloads from backend state', () => {
      const downloads = useDownloadStore()

      downloads.hydrateFromBackend({ 'model-a': 0.3, 'model-b': 0.8 })

      expect(Object.keys(downloads.activeDownloads)).toEqual(['model-a', 'model-b'])
      expect(downloads.activeDownloads['model-a']!.progress).toBe(0.3)
      expect(downloads.activeDownloads['model-b']!.progress).toBe(0.8)
    })

    it('preserves stopping state from existing entries', () => {
      const downloads = useDownloadStore()

      downloads.activeDownloads = { 'model-a': { progress: 0.2, stopping: true, downloaded: 0, totalSize: 0, speed: 0 } }
      downloads.hydrateFromBackend({ 'model-a': 0.5 })

      expect(downloads.activeDownloads['model-a']!.stopping).toBe(true)
      expect(downloads.activeDownloads['model-a']!.progress).toBe(0.5)
    })
  })

  describe('progress event listener', () => {
    it('updates progress from event', () => {
      const downloads = useDownloadStore()
      downloads.setupListeners()

      downloads.activeDownloads = { 'whisper:tiny': { progress: 0.2, stopping: false, downloaded: 20_000_000, totalSize: 100_000_000, speed: 0 } }

      // Simulate event
      listeners['download-progress']?.({
        payload: { model_id: 'whisper:tiny', progress: 0.5, downloaded: 50_000_000, total_size: 100_000_000, speed: 2_000_000 },
      })

      expect(downloads.activeDownloads['whisper:tiny']!.progress).toBe(0.5)
      expect(downloads.activeDownloads['whisper:tiny']!.downloaded).toBe(50_000_000)
      expect(downloads.activeDownloads['whisper:tiny']!.speed).toBe(2_000_000)
    })

    it('does not regress progress', () => {
      const downloads = useDownloadStore()
      downloads.setupListeners()

      downloads.activeDownloads = { 'whisper:tiny': { progress: 0.8, stopping: false, downloaded: 80_000_000, totalSize: 100_000_000, speed: 1_000_000 } }

      listeners['download-progress']?.({
        payload: { model_id: 'whisper:tiny', progress: 0.5, downloaded: 50_000_000, total_size: 100_000_000, speed: 500_000 },
      })

      // Should NOT go backwards
      expect(downloads.activeDownloads['whisper:tiny']!.progress).toBe(0.8)
    })

    it('ignores events for unknown models', () => {
      const downloads = useDownloadStore()
      downloads.setupListeners()

      listeners['download-progress']?.({
        payload: { model_id: 'unknown', progress: 0.5, downloaded: 50_000_000, total_size: 100_000_000, speed: 1_000_000 },
      })

      expect(downloads.activeDownloads['unknown']).toBeUndefined()
    })
  })
})
