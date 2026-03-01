import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { HistoryEntry } from './types'

export const useHistoryStore = defineStore('history', () => {
  const history = ref<HistoryEntry[]>([])

  async function fetchHistory() {
    try { history.value = await invoke('get_history') }
    catch (e) { console.error('fetchHistory failed:', e) }
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

  function addEntry(entry: HistoryEntry) {
    history.value.unshift(entry)
  }

  return {
    history,
    fetchHistory, clearHistoryAction, searchHistory,
    deleteHistoryEntry, deleteHistoryDay, addEntry,
  }
})
