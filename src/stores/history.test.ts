import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const mockInvoke = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}))
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

import { useHistoryStore } from './history'
import type { HistoryEntry } from './types'

function makeEntry(overrides: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    text: 'hello world',
    timestamp: 1700000000,
    model_id: 'whisper:large-v3',
    language: 'en',
    cleanup_model_id: '',
    hallucination_filter: false,
    vad_trimmed: false,
    punctuation_model_id: '',
    spellcheck: false,
    disfluency_removal: false,
    itn: false,
    raw_text: '',
    word_scores: '',
    rating: 0,
    ...overrides,
  }
}

describe('history store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockInvoke.mockReset()
  })

  describe('fetchHistory', () => {
    it('populates history and total on initial load', async () => {
      const store = useHistoryStore()
      mockInvoke.mockResolvedValueOnce({
        entries: [makeEntry({ timestamp: 100 }), makeEntry({ timestamp: 99 })],
        total: 5,
      })

      await store.fetchHistory()

      expect(store.history).toHaveLength(2)
      expect(store.total).toBe(5)
      expect(store.hasMore).toBe(true)
    })

    it('appends entries when append=true and uses cursor', async () => {
      const store = useHistoryStore()
      store.history = [makeEntry({ timestamp: 100 })]
      store.total = 3

      mockInvoke.mockResolvedValueOnce({
        entries: [makeEntry({ timestamp: 50 })],
        total: 3,
      })

      await store.fetchHistory('', true)

      expect(store.history).toHaveLength(2)
      // Should have used cursor from last entry
      expect(mockInvoke).toHaveBeenCalledWith('get_history', {
        query: '',
        limit: 50,
        cursor: 100,
      })
    })

    it('sets query on non-append fetch', async () => {
      const store = useHistoryStore()
      mockInvoke.mockResolvedValueOnce({ entries: [], total: 0 })

      await store.fetchHistory('search term')

      expect(mockInvoke).toHaveBeenCalledWith('get_history', {
        query: 'search term',
        limit: 50,
        cursor: null,
      })
    })
  })

  describe('loadMore', () => {
    it('is a no-op when hasMore is false', async () => {
      const store = useHistoryStore()
      store.hasMore = false

      await store.loadMore()

      expect(mockInvoke).not.toHaveBeenCalled()
    })
  })

  describe('deleteHistoryEntry', () => {
    it('removes entry and decrements total', async () => {
      const store = useHistoryStore()
      store.history = [makeEntry({ timestamp: 100 }), makeEntry({ timestamp: 200 })]
      store.total = 5
      mockInvoke.mockResolvedValueOnce(undefined)

      await store.deleteHistoryEntry(100)

      expect(store.history).toHaveLength(1)
      expect(store.history[0]!.timestamp).toBe(200)
      expect(store.total).toBe(4)
    })
  })

  describe('setRating', () => {
    it('stores the verdict and persists it', async () => {
      const store = useHistoryStore()
      store.history = [makeEntry({ timestamp: 100 })]
      mockInvoke.mockResolvedValueOnce(undefined)

      await store.setRating(100, -1)

      expect(store.history[0]!.rating).toBe(-1)
      expect(mockInvoke).toHaveBeenCalledWith('set_history_rating', { timestamp: 100, rating: -1 })
    })

    it('rolls back when the backend rejects', async () => {
      const store = useHistoryStore()
      store.history = [makeEntry({ timestamp: 100, rating: 1 })]
      mockInvoke.mockRejectedValueOnce(new Error('nope'))

      await store.setRating(100, -1)

      expect(store.history[0]!.rating).toBe(1)
    })
  })

  describe('setCorrection', () => {
    it('replaces the text and appends a manual step', async () => {
      const store = useHistoryStore()
      store.history = [makeEntry({ timestamp: 100, text: 'bonjour le mode', raw_text: '[["asr","bonjour le mode"]]' })]
      mockInvoke.mockResolvedValueOnce(undefined)

      await store.setCorrection(100, 'bonjour le monde')

      expect(store.history[0]!.text).toBe('bonjour le monde')
      const steps = JSON.parse(store.history[0]!.raw_text) as [string, string][]
      expect(steps[0]).toEqual(['asr', 'bonjour le mode'])
      expect(steps[steps.length - 1]).toEqual(['manual', 'bonjour le monde'])
    })

    it('keeps a single manual step when corrected twice', async () => {
      const store = useHistoryStore()
      store.history = [makeEntry({ timestamp: 100, raw_text: '[["asr","a"],["manual","b"]]' })]
      mockInvoke.mockResolvedValueOnce(undefined)

      await store.setCorrection(100, 'c')

      const steps = JSON.parse(store.history[0]!.raw_text) as [string, string][]
      expect(steps.filter(([n]) => n === 'manual')).toHaveLength(1)
      expect(steps[steps.length - 1]).toEqual(['manual', 'c'])
    })
  })

  describe('deleteHistoryDay', () => {
    it('filters entries of the day and adjusts total', async () => {
      const store = useHistoryStore()
      const dayStart = 1700000000
      store.history = [
        makeEntry({ timestamp: dayStart + 100 }),
        makeEntry({ timestamp: dayStart + 200 }),
        makeEntry({ timestamp: dayStart + 86500 }), // next day
      ]
      store.total = 10
      mockInvoke.mockResolvedValueOnce(undefined)

      await store.deleteHistoryDay(dayStart)

      expect(store.history).toHaveLength(1)
      expect(store.history[0]!.timestamp).toBe(dayStart + 86500)
      expect(store.total).toBe(8)
    })
  })

  describe('clearHistoryAction', () => {
    it('resets everything', async () => {
      const store = useHistoryStore()
      store.history = [makeEntry()]
      store.total = 10
      store.hasMore = true
      mockInvoke.mockResolvedValueOnce(undefined)

      await store.clearHistoryAction()

      expect(store.history).toHaveLength(0)
      expect(store.total).toBe(0)
      expect(store.hasMore).toBe(false)
    })
  })

  describe('addEntry', () => {
    it('prepends entry when no search filter', () => {
      const store = useHistoryStore()
      store.history = [makeEntry({ timestamp: 100 })]
      store.total = 1

      store.addEntry(makeEntry({ timestamp: 200, text: 'new' }))

      expect(store.history).toHaveLength(2)
      expect(store.history[0]!.text).toBe('new')
      expect(store.total).toBe(2)
    })

    it('does not prepend when query is active', async () => {
      const store = useHistoryStore()
      mockInvoke.mockResolvedValueOnce({ entries: [], total: 0 })
      await store.fetchHistory('search')

      store.addEntry(makeEntry({ text: 'new' }))

      expect(store.history).toHaveLength(0)
    })
  })
})
