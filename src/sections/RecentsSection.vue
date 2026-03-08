<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { useHistoryStore } from '@/stores/history'
import type { HistoryEntry } from '@/stores/types'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import HistoryEntryCard from '@/components/HistoryEntryCard.vue'
import { Search } from 'lucide-vue-next'

const { t } = useI18n()
const historyStore = useHistoryStore()

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

onMounted(() => {
  if (historyStore.history.length === 0) {
    historyStore.fetchHistory()
  }
})

// -- Grouping + flattening --

interface DayGroup {
  label: string
  dayTimestamp: number
  entries: HistoryEntry[]
}

type FlatItem =
  | { kind: 'day-header'; label: string; dayTimestamp: number }
  | { kind: 'entry'; entry: HistoryEntry }

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

const flatItems = computed<FlatItem[]>(() => {
  const items: FlatItem[] = []
  for (const group of groupedHistory.value) {
    items.push({ kind: 'day-header', label: group.label, dayTimestamp: group.dayTimestamp })
    for (const entry of group.entries) {
      items.push({ kind: 'entry', entry })
    }
  }
  return items
})

// -- Virtual scroll --

const scrollEl = ref<HTMLElement | null>(null)

const virtualizer = useVirtualizer(computed(() => ({
  count: flatItems.value.length,
  getScrollElement: () => scrollEl.value,
  estimateSize: (index: number) => flatItems.value[index]?.kind === 'day-header' ? 32 : 80,
  overscan: 5,
})))

// Infinite scroll: detect when near bottom
function onScroll() {
  const el = scrollEl.value
  if (!el || !historyStore.hasMore) return
  const { scrollTop, scrollHeight, clientHeight } = el
  if (scrollHeight - scrollTop - clientHeight < 200) {
    historyStore.loadMore()
  }
}

// -- Helpers --

function dateKey(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

async function copyEntry(entry: HistoryEntry) {
  await navigator.clipboard.writeText(entry.text)
  copiedTimestamp.value = entry.timestamp
  setTimeout(() => {
    if (copiedTimestamp.value === entry.timestamp) copiedTimestamp.value = null
  }, 1500)
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

function entryAt(index: number): HistoryEntry {
  return (flatItems.value[index] as { kind: 'entry'; entry: HistoryEntry }).entry
}

function headerAt(index: number) {
  return flatItems.value[index] as { kind: 'day-header'; label: string; dayTimestamp: number }
}
</script>

<template>
  <div class="flex flex-col h-full">
    <!-- Header: title + clear all -->
    <div class="flex items-center justify-between mb-0">
      <div class="text-[20px] font-bold tracking-[-0.02em]">{{ t('panel.recents') }}</div>
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

      <!-- Virtual-scrolled timeline -->
      <div v-else ref="scrollEl" class="h-full overflow-auto" @scroll="onScroll">
        <div :style="{ height: `${virtualizer.getTotalSize()}px`, position: 'relative' }">
          <div
            v-for="vItem in virtualizer.getVirtualItems()"
            :key="vItem.index"
            :data-index="vItem.index"
            :ref="(el) => virtualizer.measureElement(el as Element)"
            :style="{ position: 'absolute', top: 0, left: 0, width: '100%', transform: `translateY(${vItem.start}px)` }"
          >
            <!-- Day header -->
            <template v-if="flatItems[vItem.index]!.kind === 'day-header'">
              <div class="flex items-center justify-between mb-1.5 group/day" :class="{ 'mt-3.5': vItem.index > 0 }">
                <span class="text-[11px] font-semibold text-muted-foreground uppercase tracking-wide">
                  {{ headerAt(vItem.index).label }}
                </span>
                <button
                  class="opacity-0 group-hover/day:opacity-100 transition-opacity duration-150 text-[11px] text-muted-foreground hover:text-destructive px-1.5 py-0.5 rounded cursor-pointer"
                  @click="confirmDeleteDay(headerAt(vItem.index).dayTimestamp)"
                >
                  {{ t('history.deleteDay') }}
                </button>
              </div>
            </template>

            <!-- Entry card -->
            <template v-else>
              <HistoryEntryCard
                :entry="entryAt(vItem.index)"
                :copied-timestamp="copiedTimestamp"
                @copy="copyEntry"
                @delete="deleteEntry"
              />
            </template>
          </div>
        </div>
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
