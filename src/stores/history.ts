import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { HistoryEntry } from './types'

interface HistoryPage {
  entries: HistoryEntry[]
  total: number
}

const PAGE_SIZE = 50

export const useHistoryStore = defineStore('history', () => {
  const history = ref<HistoryEntry[]>([])
  const total = ref(0)
  const query = ref('')
  const hasMore = ref(false)

  async function fetchHistory(searchQuery = '', append = false) {
    try {
      const offset = append ? history.value.length : 0
      if (!append) query.value = searchQuery
      const page = await invoke<HistoryPage>('get_history', {
        query: query.value,
        limit: PAGE_SIZE,
        offset,
      })
      if (append) {
        history.value.push(...page.entries)
      } else {
        history.value = page.entries
      }
      total.value = page.total
      hasMore.value = history.value.length < page.total
    } catch (e) { console.error('fetchHistory failed:', e) }
  }

  async function loadMore() {
    if (!hasMore.value) return
    await fetchHistory('', true)
  }

  async function clearHistoryAction() {
    try {
      await invoke('clear_history')
      history.value = []
      total.value = 0
      hasMore.value = false
    } catch (e) { console.error('clearHistory failed:', e) }
  }

  async function deleteHistoryEntry(timestamp: number) {
    try {
      await invoke('delete_history_entry', { timestamp: Math.floor(timestamp) })
      history.value = history.value.filter(e => e.timestamp !== timestamp)
      total.value = Math.max(0, total.value - 1)
    } catch (e) { console.error('deleteHistoryEntry failed:', e) }
  }

  async function deleteHistoryDay(dayTimestamp: number) {
    try {
      await invoke('delete_history_day', { dayTimestamp: Math.floor(dayTimestamp) })
      const dayEnd = dayTimestamp + 86400
      const removed = history.value.filter(e => e.timestamp >= dayTimestamp && e.timestamp < dayEnd).length
      history.value = history.value.filter(e => e.timestamp < dayTimestamp || e.timestamp >= dayEnd)
      total.value = Math.max(0, total.value - removed)
    } catch (e) { console.error('deleteHistoryDay failed:', e) }
  }

  function addEntry(entry: HistoryEntry) {
    // Only prepend if no search filter active (otherwise it wouldn't match)
    if (!query.value) {
      history.value.unshift(entry)
      total.value++
    }
  }

  return {
    history, total, hasMore,
    fetchHistory, loadMore, clearHistoryAction,
    deleteHistoryEntry, deleteHistoryDay, addEntry,
  }
})
