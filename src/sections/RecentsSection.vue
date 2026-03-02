<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useHistoryStore } from '@/stores/history'
import { useEnginesStore } from '@/stores/engines'
import { parseCloudId } from '@/stores/types'
import type { HistoryEntry } from '@/stores/types'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Search, Copy, Check, Trash2 } from 'lucide-vue-next'
import TypeBadge from '@/components/TypeBadge.vue'

const { t } = useI18n()
const historyStore = useHistoryStore()
const enginesStore = useEnginesStore()

const searchQuery = ref('')
const copiedTimestamp = ref<number | null>(null)
const showClearAllConfirm = ref(false)
const showDeleteDayConfirm = ref(false)
const deleteDayTarget = ref<number>(0)

// Debounced search
let searchTimeout: ReturnType<typeof setTimeout> | null = null
watch(searchQuery, () => {
  if (searchTimeout) clearTimeout(searchTimeout)
  searchTimeout = setTimeout(() => {
    historyStore.fetchHistory(searchQuery.value)
  }, 250)
})

// Infinite scroll via IntersectionObserver
const sentinel = ref<HTMLElement | null>(null)
let observer: IntersectionObserver | null = null

onMounted(() => {
  observer = new IntersectionObserver((entries) => {
    if (entries[0]?.isIntersecting && historyStore.hasMore) {
      historyStore.loadMore()
    }
  }, { threshold: 0 })
})

watch(sentinel, (el, oldEl) => {
  if (oldEl && observer) observer.unobserve(oldEl)
  if (el && observer) observer.observe(el)
})

onUnmounted(() => {
  observer?.disconnect()
})

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

  for (const entry of historyStore.history) {
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
    if (copiedTimestamp.value === entry.timestamp) copiedTimestamp.value = null
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
  const model = enginesStore.models.find(m => m.id === id)
  return model ? model.label : id
}

function cleanupBadgeType(id: string): 'bert' | 'correction' | 'llm' | 'cloud' {
  if (id.startsWith('bert-punctuation:') || id.startsWith('pcs-punctuation:')) return 'bert'
  if (id.startsWith('correction:')) return 'correction'
  if (parseCloudId(id)) return 'cloud'
  return 'llm'
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
  <div class="flex flex-col h-full">
    <!-- Header: title + clear all -->
    <div class="flex items-center justify-between mb-0">
      <div class="section-title" style="margin-bottom: 0;">{{ t('panel.recents') }}</div>
      <Button
        v-if="historyStore.history.length > 0"
        variant="destructive"
        size="sm"
        class="shrink-0 h-7 text-[11px]"
        @click="showClearAllConfirm = true"
      >
        {{ t('history.clearAll') }}
      </Button>
    </div>

    <!-- Search bar -->
    <div class="relative mt-2.5 mb-3.5">
      <Search class="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground pointer-events-none" />
      <Input
        v-model="searchQuery"
        :placeholder="t('history.search')"
        class="h-8 pl-9 text-[13px]"
      />
    </div>

    <!-- Content -->
    <div class="flex-1 min-h-0">
      <!-- Empty state -->
      <div v-if="historyStore.total === 0 && !searchQuery" class="flex items-center justify-center h-40 text-muted-foreground text-sm">
        {{ t('history.empty') }}
      </div>

      <!-- Empty search -->
      <div v-else-if="historyStore.history.length === 0 && searchQuery" class="flex items-center justify-center h-40 text-muted-foreground text-sm">
        {{ t('history.emptySearch', [searchQuery]) }}
      </div>

      <!-- Timeline -->
      <div v-else class="space-y-3.5">
        <div v-for="group in groupedHistory" :key="group.dayTimestamp" class="wf-day-group">
          <div class="flex items-center justify-between mb-1.5">
            <span class="text-[11px] font-semibold text-muted-foreground uppercase tracking-wide">
              {{ group.label }}
            </span>
            <button
              class="wf-day-delete text-[11px] text-muted-foreground hover:text-destructive px-1.5 py-0.5 rounded cursor-pointer"
              @click="confirmDeleteDay(group.dayTimestamp)"
            >
              {{ t('history.deleteDay') }}
            </button>
          </div>

          <!-- History items as individual cards -->
          <div class="space-y-1.5">
            <div
              v-for="entry in group.entries"
              :key="entry.timestamp"
              class="wf-history-item group"
            >
              <span class="text-[11px] text-muted-foreground mt-0.5 shrink-0 tabular-nums min-w-[38px]">
                {{ formatTime(entry.timestamp) }}
              </span>
              <div class="flex-1 min-w-0">
                <p class="text-[13px] leading-snug line-clamp-2 mb-1">{{ entry.text }}</p>
                <TooltipProvider v-if="entry.model_id" :delay-duration="300">
                  <div class="flex flex-wrap gap-1">
                    <Tooltip>
                      <TooltipTrigger as-child>
                        <TypeBadge :type="isCloudAsr(entry.model_id) ? 'cloud' : 'local'">
                          {{ formatAsrLabel(entry.model_id) }}
                        </TypeBadge>
                      </TooltipTrigger>
                      <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.asr') }}</TooltipContent>
                    </Tooltip>
                    <Tooltip v-if="entry.language">
                      <TooltipTrigger as-child>
                        <span class="inline-flex items-center rounded px-1.5 py-0.5 text-[10px] font-medium bg-zinc-500/10 text-zinc-600 dark:text-zinc-400">
                          {{ entry.language }}
                        </span>
                      </TooltipTrigger>
                      <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.language') }}</TooltipContent>
                    </Tooltip>
                    <Tooltip v-if="entry.vad_trimmed">
                      <TooltipTrigger as-child>
                        <TypeBadge type="vad">VAD</TypeBadge>
                      </TooltipTrigger>
                      <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.vad') }}</TooltipContent>
                    </Tooltip>
                    <Tooltip v-if="entry.cleanup_model_id">
                      <TooltipTrigger as-child>
                        <TypeBadge :type="cleanupBadgeType(entry.cleanup_model_id)">
                          {{ formatCleanupLabel(entry.cleanup_model_id) }}
                        </TypeBadge>
                      </TooltipTrigger>
                      <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.cleanup') }}</TooltipContent>
                    </Tooltip>
                    <Tooltip v-if="entry.hallucination_filter">
                      <TooltipTrigger as-child>
                        <TypeBadge type="hallucination" />
                      </TooltipTrigger>
                      <TooltipContent side="bottom" :side-offset="4">{{ t('history.badge.hallucination') }}</TooltipContent>
                    </Tooltip>
                  </div>
                </TooltipProvider>
              </div>
              <div class="flex gap-1 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity pt-0.5">
                <TooltipProvider :delay-duration="300">
                  <Tooltip>
                    <TooltipTrigger as-child>
                      <button class="w-6 h-6 flex items-center justify-center rounded text-muted-foreground hover:text-foreground hover:bg-muted/50" @click="copyEntry(entry)">
                        <Check v-if="copiedTimestamp === entry.timestamp" class="h-3.5 w-3.5 text-green-600" />
                        <Copy v-else class="h-3.5 w-3.5" />
                      </button>
                    </TooltipTrigger>
                    <TooltipContent side="bottom" :side-offset="4">{{ t('history.copy') }}</TooltipContent>
                  </Tooltip>
                  <Tooltip>
                    <TooltipTrigger as-child>
                      <button class="w-6 h-6 flex items-center justify-center rounded text-muted-foreground hover:text-destructive hover:bg-muted/50" @click="deleteEntry(entry)">
                        <Trash2 class="h-3.5 w-3.5" />
                      </button>
                    </TooltipTrigger>
                    <TooltipContent side="bottom" :side-offset="4">{{ t('history.delete') }}</TooltipContent>
                  </Tooltip>
                </TooltipProvider>
              </div>
            </div>
          </div>
        </div>

        <!-- Sentinel for infinite scroll -->
        <div ref="sentinel" class="h-1" />
      </div>
    </div>

    <!-- Clear All confirmation -->
    <ConfirmDialog
      v-model:open="showClearAllConfirm"
      :title="t('history.clearAllConfirm')"
      :description="t('history.clearAllDesc')"
      :confirm-label="t('history.clearAll')"
      @confirm="doClearAll"
    />

    <!-- Delete Day confirmation -->
    <ConfirmDialog
      v-model:open="showDeleteDayConfirm"
      :title="t('history.deleteDayConfirm')"
      :confirm-label="t('history.deleteDay')"
      @confirm="doDeleteDay"
    />
  </div>
</template>
