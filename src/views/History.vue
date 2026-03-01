<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '@/stores/app'
import { useHistoryStore } from '@/stores/history'
import { useEnginesStore } from '@/stores/engines'
import { parseCloudId } from '@/stores/types'
import type { HistoryEntry } from '@/stores/types'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog'
import { Search, Copy, Check, Trash2, Cloud, Cpu, ShieldCheck, SpellCheck } from 'lucide-vue-next'

const { t } = useI18n()
const store = useAppStore()
const historyStore = useHistoryStore()
const enginesStore = useEnginesStore()

const searchQuery = ref('')
const copiedTimestamp = ref<number | null>(null)
const showClearAllConfirm = ref(false)
const showDeleteDayConfirm = ref(false)
const deleteDayTarget = ref<number>(0)

// Debounced search
let searchTimeout: ReturnType<typeof setTimeout> | null = null
const filteredHistory = ref<HistoryEntry[]>([])

function updateFiltered() {
  const q = searchQuery.value.toLowerCase()
  if (!q) {
    filteredHistory.value = historyStore.history
  } else {
    filteredHistory.value = historyStore.history.filter(e => e.text.toLowerCase().includes(q))
  }
}

watch(() => historyStore.history, updateFiltered, { deep: true })
watch(searchQuery, () => {
  if (searchTimeout) clearTimeout(searchTimeout)
  searchTimeout = setTimeout(updateFiltered, 150)
})

onMounted(async () => {
  getCurrentWindow().setTitle(t('window.history'))
  await store.init()
  updateFiltered()
})

// Group entries by day
interface DayGroup {
  label: string
  dayTimestamp: number
  entries: HistoryEntry[]
}

const groupedHistory = computed<DayGroup[]>(() => {
  const groups = new Map<string, DayGroup>()
  const now = new Date()
  const todayKey = dateKey(now)
  const yesterday = new Date(now)
  yesterday.setDate(yesterday.getDate() - 1)
  const yesterdayKey = dateKey(yesterday)

  for (const entry of filteredHistory.value) {
    const date = new Date(entry.timestamp * 1000)
    const key = dateKey(date)

    if (!groups.has(key)) {
      let label: string
      if (key === todayKey) {
        label = t('history.today')
      } else if (key === yesterdayKey) {
        label = t('history.yesterday')
      } else {
        label = date.toLocaleDateString(undefined, { day: 'numeric', month: 'long', year: 'numeric' })
      }

      // Compute midnight UTC for this day
      const dayDate = new Date(date.getFullYear(), date.getMonth(), date.getDate())
      const dayTimestamp = Math.floor(dayDate.getTime() / 1000)

      groups.set(key, { label, dayTimestamp, entries: [] })
    }
    groups.get(key)!.entries.push(entry)
  }

  return Array.from(groups.values())
})

function dateKey(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

function formatTime(timestamp: number): string {
  const date = new Date(timestamp * 1000)
  return date.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
}

async function copyEntry(entry: HistoryEntry) {
  await navigator.clipboard.writeText(entry.text)
  copiedTimestamp.value = entry.timestamp
  setTimeout(() => {
    if (copiedTimestamp.value === entry.timestamp) {
      copiedTimestamp.value = null
    }
  }, 1500)
}

function formatAsrLabel(modelId: string): string {
  const cloudId = parseCloudId(modelId)
  if (cloudId) {
    const provider = enginesStore.providers.find(p => p.id === cloudId)
    return provider ? provider.name : 'Cloud'
  }
  const model = enginesStore.models.find(m => m.id === modelId)
  return model ? model.label : modelId
}

function isCloudAsr(modelId: string): boolean {
  return !!parseCloudId(modelId)
}

function formatCleanupLabel(id: string): string {
  if (id.startsWith('bert-punctuation:')) return 'BERT'
  const cloudId = parseCloudId(id)
  if (cloudId) {
    const provider = enginesStore.providers.find(p => p.id === cloudId)
    return provider ? provider.name : 'Cloud LLM'
  }
  // Local LLM (llama:*)
  const model = enginesStore.models.find(m => m.id === id)
  return model ? model.label : id
}

function cleanupBadgeType(id: string): 'bert' | 'cloud' | 'local' {
  if (id.startsWith('bert-punctuation:')) return 'bert'
  if (parseCloudId(id)) return 'cloud'
  return 'local'
}

async function deleteEntry(entry: HistoryEntry) {
  await historyStore.deleteHistoryEntry(entry.timestamp)
}

function confirmDeleteDay(dayTimestamp: number) {
  deleteDayTarget.value = dayTimestamp
  showDeleteDayConfirm.value = true
}

async function doDeleteDay() {
  showDeleteDayConfirm.value = false
  await historyStore.deleteHistoryDay(deleteDayTarget.value)
}

async function doClearAll() {
  showClearAllConfirm.value = false
  await historyStore.clearHistoryAction()
}
</script>

<template>
  <div class="flex flex-col h-full select-none">
    <!-- Header -->
    <div class="flex items-center justify-between px-5 pt-5 pb-2">
      <h1 class="text-lg font-semibold">{{ t('history.title') }}</h1>
      <Button
        v-if="historyStore.history.length > 0"
        variant="ghost"
        size="sm"
        class="text-destructive hover:text-destructive"
        @click="showClearAllConfirm = true"
      >
        {{ t('history.clearAll') }}
      </Button>
    </div>

    <!-- Search -->
    <div class="relative px-5 mb-3">
      <Search class="absolute left-7 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground pointer-events-none" />
      <Input
        v-model="searchQuery"
        :placeholder="t('history.search')"
        class="h-9 pl-8 text-sm"
      />
    </div>

    <!-- Content -->
    <div class="flex-1 overflow-y-auto px-5 pb-5">
      <!-- Empty state -->
      <div v-if="historyStore.history.length === 0" class="flex items-center justify-center h-full text-muted-foreground text-sm">
        {{ t('history.empty') }}
      </div>

      <!-- Empty search -->
      <div v-else-if="filteredHistory.length === 0 && searchQuery" class="flex items-center justify-center h-full text-muted-foreground text-sm">
        {{ t('history.emptySearch', [searchQuery]) }}
      </div>

      <!-- Timeline -->
      <div v-else class="space-y-4">
        <div v-for="group in groupedHistory" :key="group.dayTimestamp">
          <!-- Day header -->
          <div class="flex items-center justify-between mb-2">
            <span class="text-xs font-medium text-muted-foreground uppercase tracking-wide">
              {{ group.label }}
            </span>
            <Button
              variant="ghost"
              size="sm"
              class="h-6 text-xs text-muted-foreground hover:text-destructive px-2"
              @click="confirmDeleteDay(group.dayTimestamp)"
            >
              {{ t('history.deleteDay') }}
            </Button>
          </div>

          <!-- Entries -->
          <div class="rounded-lg border border-border divide-y divide-border">
            <div
              v-for="entry in group.entries"
              :key="entry.timestamp"
              class="px-3 py-2 group"
            >
              <div class="flex items-start gap-2">
                <span class="text-xs text-muted-foreground mt-0.5 shrink-0 tabular-nums">
                  {{ formatTime(entry.timestamp) }}
                </span>
                <p class="text-sm flex-1 min-w-0 break-words">{{ entry.text }}</p>
                <div class="flex gap-1 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">
                  <Button
                    variant="ghost"
                    size="sm"
                    class="h-6 w-6 p-0"
                    :title="t('history.copy')"
                    @click="copyEntry(entry)"
                  >
                    <Check v-if="copiedTimestamp === entry.timestamp" class="h-3.5 w-3.5 text-green-600" />
                    <Copy v-else class="h-3.5 w-3.5" />
                  </Button>
                  <Button
                    variant="ghost"
                    size="sm"
                    class="h-6 w-6 p-0 hover:text-destructive"
                    :title="t('history.delete')"
                    @click="deleteEntry(entry)"
                  >
                    <Trash2 class="h-3.5 w-3.5" />
                  </Button>
                </div>
              </div>
              <div v-if="entry.model_id" class="ml-12 mt-1 flex flex-wrap gap-1">
                <!-- ASR badge -->
                <span
                  class="inline-flex items-center gap-0.5 rounded-full px-1.5 py-0.5 text-[10px] font-medium"
                  :class="isCloudAsr(entry.model_id)
                    ? 'bg-sky-500/10 text-sky-600 dark:text-sky-400'
                    : 'bg-zinc-500/10 text-zinc-600 dark:text-zinc-400'"
                >
                  <Cloud v-if="isCloudAsr(entry.model_id)" class="h-2.5 w-2.5" />
                  <Cpu v-else class="h-2.5 w-2.5" />
                  {{ formatAsrLabel(entry.model_id) }}
                </span>
                <!-- Language badge -->
                <span v-if="entry.language" class="inline-flex items-center rounded-full px-1.5 py-0.5 text-[10px] font-medium bg-zinc-500/10 text-zinc-600 dark:text-zinc-400">
                  {{ entry.language }}
                </span>
                <!-- Cleanup badge -->
                <span
                  v-if="entry.cleanup_model_id"
                  class="inline-flex items-center gap-0.5 rounded-full px-1.5 py-0.5 text-[10px] font-medium"
                  :class="{
                    'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400': cleanupBadgeType(entry.cleanup_model_id) === 'bert',
                    'bg-violet-500/10 text-violet-600 dark:text-violet-400': cleanupBadgeType(entry.cleanup_model_id) === 'local',
                    'bg-amber-500/10 text-amber-600 dark:text-amber-400': cleanupBadgeType(entry.cleanup_model_id) === 'cloud',
                  }"
                >
                  <SpellCheck v-if="cleanupBadgeType(entry.cleanup_model_id) === 'bert'" class="h-2.5 w-2.5" />
                  <Cpu v-else-if="cleanupBadgeType(entry.cleanup_model_id) === 'local'" class="h-2.5 w-2.5" />
                  <Cloud v-else class="h-2.5 w-2.5" />
                  {{ formatCleanupLabel(entry.cleanup_model_id) }}
                </span>
                <!-- Hallucination filter badge -->
                <span
                  v-if="entry.hallucination_filter"
                  class="inline-flex items-center gap-0.5 rounded-full px-1.5 py-0.5 text-[10px] font-medium bg-rose-500/10 text-rose-600 dark:text-rose-400"
                >
                  <ShieldCheck class="h-2.5 w-2.5" />
                  Anti-halluc.
                </span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Clear All confirmation -->
    <AlertDialog :open="showClearAllConfirm" @update:open="showClearAllConfirm = $event">
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{{ t('history.clearAllConfirm') }}</AlertDialogTitle>
          <AlertDialogDescription>{{ t('history.clearAllDesc') }}</AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel @click="showClearAllConfirm = false">{{ t('history.cancel') }}</AlertDialogCancel>
          <AlertDialogAction @click="doClearAll" class="bg-destructive text-destructive-foreground hover:bg-destructive/90">
            {{ t('history.clearAll') }}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>

    <!-- Delete Day confirmation -->
    <AlertDialog :open="showDeleteDayConfirm" @update:open="showDeleteDayConfirm = $event">
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{{ t('history.deleteDayConfirm') }}</AlertDialogTitle>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel @click="showDeleteDayConfirm = false">{{ t('history.cancel') }}</AlertDialogCancel>
          <AlertDialogAction @click="doDeleteDay" class="bg-destructive text-destructive-foreground hover:bg-destructive/90">
            {{ t('history.deleteDay') }}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  </div>
</template>
