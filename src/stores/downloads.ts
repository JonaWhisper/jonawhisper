import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { listen, type Event } from '@tauri-apps/api/event'
import { useEnginesStore } from './engines'
import type { DownloadProgressPayload } from './types'

export const useDownloadStore = defineStore('downloads', () => {
  const enginesStore = useEnginesStore()

  // State
  const activeDownloads = ref<Record<string, { progress: number; stopping: boolean; downloaded: number; totalSize: number; speed: number }>>({})
  const deletingModels = ref<Record<string, boolean>>({})

  // Actions
  async function downloadModel(id: string) {
    if (activeDownloads.value[id]) return false
    const model = enginesStore.models.find(m => m.id === id)
    const initialProgress = model?.partial_progress ?? 0
    activeDownloads.value = { ...activeDownloads.value, [id]: { progress: initialProgress, stopping: false, downloaded: 0, totalSize: model?.size ?? 0, speed: 0 } }
    try {
      const success = await invoke<boolean>('download_model_cmd', { id })
      return success
    } catch (e) {
      console.error('downloadModel failed:', e)
      return false
    } finally {
      const entry = activeDownloads.value[id]
      if (entry) {
        const lastProgress = entry.progress
        const m = enginesStore.models.find(m => m.id === id)
        if (m) {
          if (lastProgress >= 1) {
            m.is_downloaded = true
            m.partial_progress = null
          } else if (lastProgress > 0) {
            m.partial_progress = lastProgress
          } else {
            m.partial_progress = null
          }
        }
        const { [id]: _, ...rest } = activeDownloads.value
        activeDownloads.value = rest
      }
      enginesStore.fetchModels()
    }
  }

  async function pauseDownload(id: string) {
    const entry = activeDownloads.value[id]
    if (!entry) return
    // Mark as stopping so UI shows spinner instead of controls
    activeDownloads.value = { ...activeDownloads.value, [id]: { ...entry, stopping: true } }
    try { await invoke('pause_download', { id }) }
    catch (e) { console.error('pauseDownload failed:', e) }
    // Save partial progress and remove from active downloads after backend confirms
    const current = activeDownloads.value[id]
    const m = enginesStore.models.find(m => m.id === id)
    if (m && current && current.progress > 0) {
      m.partial_progress = current.progress
    }
    const { [id]: _, ...rest } = activeDownloads.value
    activeDownloads.value = rest
  }

  async function cancelDownload(id: string) {
    if (activeDownloads.value[id]) {
      activeDownloads.value = { ...activeDownloads.value, [id]: { ...activeDownloads.value[id], stopping: true } }
    } else {
      const m = enginesStore.models.find(m => m.id === id)
      if (m) m.partial_progress = null
    }
    try {
      await invoke('cancel_download', { id })
      await enginesStore.fetchModels()
    } catch (e) { console.error('cancelDownload failed:', e) }
  }

  async function deleteModel(id: string) {
    deletingModels.value = { ...deletingModels.value, [id]: true }
    try {
      const success = await invoke<boolean>('delete_model_cmd', { id })
      if (success) {
        await enginesStore.fetchModels()
      }
      return success
    } catch (e) {
      console.error('deleteModel failed:', e)
      return false
    } finally {
      const { [id]: _, ...rest } = deletingModels.value
      deletingModels.value = rest
    }
  }

  function hydrateFromBackend(downloads: Record<string, number>) {
    const hydrated: typeof activeDownloads.value = {}
    for (const [id, progress] of Object.entries(downloads ?? {})) {
      hydrated[id] = { progress, stopping: activeDownloads.value[id]?.stopping ?? false, downloaded: 0, totalSize: 0, speed: 0 }
    }
    activeDownloads.value = hydrated
  }

  // Listeners
  function setupListeners() {
    listen<DownloadProgressPayload>('download-progress', (event: Event<DownloadProgressPayload>) => {
      const { model_id, progress, downloaded, total_size, speed } = event.payload ?? {}
      if (model_id && progress !== undefined && activeDownloads.value[model_id]) {
        const entry = activeDownloads.value[model_id]
        if (progress >= entry.progress) {
          entry.progress = progress
          if (downloaded !== undefined) entry.downloaded = downloaded
          if (total_size !== undefined) entry.totalSize = total_size
          if (speed !== undefined) entry.speed = speed
        }
      }
    })
  }

  return {
    activeDownloads, deletingModels,
    downloadModel, pauseDownload, cancelDownload, deleteModel,
    hydrateFromBackend, setupListeners,
  }
})
