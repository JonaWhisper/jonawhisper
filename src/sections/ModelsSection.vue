<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useVirtualizer } from '@tanstack/vue-virtual'
import { useEnginesStore } from '@/stores/engines'
import { useDownloadStore } from '@/stores/downloads'
import type { ASRModel } from '@/stores/types'
import ModelCell from '@/components/ModelCell.vue'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import { AudioLines, Type, SpellCheck, MessageSquare } from 'lucide-vue-next'

const { t } = useI18n()
const engines = useEnginesStore()
const downloads = useDownloadStore()

type FilterKey = 'all' | 'asr' | 'punctuation' | 'correction' | 'llm'
const activeFilter = ref<FilterKey>('all')

const filters: { key: FilterKey; label: string; icon: any; iconColor: string; activeBg: string; activeText: string }[] = [
  { key: 'all', label: 'models.filter.all', icon: null, iconColor: '', activeBg: 'bg-neutral-700 dark:bg-neutral-300', activeText: 'text-white dark:text-neutral-900' },
  { key: 'asr', label: 'models.filter.asr', icon: AudioLines, iconColor: 'bg-blue-500/15 text-blue-600 dark:text-blue-400', activeBg: 'bg-blue-500/10 dark:bg-blue-900/50', activeText: 'text-blue-700 dark:text-blue-300' },
  { key: 'punctuation', label: 'models.filter.punctuation', icon: Type, iconColor: 'bg-violet-500/15 text-violet-600 dark:text-violet-400', activeBg: 'bg-violet-500/10 dark:bg-violet-900/50', activeText: 'text-violet-700 dark:text-violet-300' },
  { key: 'correction', label: 'models.filter.correction', icon: SpellCheck, iconColor: 'bg-amber-500/15 text-amber-600 dark:text-amber-400', activeBg: 'bg-amber-500/10 dark:bg-amber-900/50', activeText: 'text-amber-700 dark:text-amber-300' },
  { key: 'llm', label: 'models.filter.llm', icon: MessageSquare, iconColor: 'bg-teal-500/15 text-teal-600 dark:text-teal-400', activeBg: 'bg-teal-500/10 dark:bg-teal-900/50', activeText: 'text-teal-700 dark:text-teal-300' },
]

const engineIdsByCategory = computed(() => {
  const map: Record<FilterKey, Set<string>> = {
    all: new Set(),
    asr: new Set(engines.asrEngines.map(e => e.id)),
    punctuation: new Set(engines.punctuationEngines.map(e => e.id)),
    correction: new Set(engines.correctionEngines.map(e => e.id)),
    llm: new Set(engines.llmEngines.map(e => e.id)),
  }
  return map
})

const filteredModels = computed(() => {
  if (activeFilter.value === 'all') return engines.models
  const ids = engineIdsByCategory.value[activeFilter.value]
  return engines.models.filter(m => ids.has(m.engine_id))
})

const showDeleteConfirm = ref(false)
const deleteTarget = ref<ASRModel | null>(null)

async function handleDownload(model: ASRModel) {
  await downloads.downloadModel(model.id)
}

function handleDeleteRequest(model: ASRModel) {
  deleteTarget.value = model
  showDeleteConfirm.value = true
}

async function confirmDelete() {
  const target = deleteTarget.value
  showDeleteConfirm.value = false
  deleteTarget.value = null
  if (target) {
    await downloads.deleteModel(target.id)
  }
}

// Virtual scroll
const scrollEl = ref<HTMLElement | null>(null)

const virtualizer = useVirtualizer(computed(() => ({
  count: filteredModels.value.length,
  getScrollElement: () => scrollEl.value,
  estimateSize: () => 68,
  overscan: 5,
})))
</script>

<template>
  <div class="flex flex-col h-full">
    <div class="text-[20px] font-bold tracking-[-0.02em] mb-4">{{ t('panel.models') }}</div>

    <!-- Filter chips -->
    <div class="flex flex-wrap gap-1 mb-3.5">
      <button
        v-for="f in filters"
        :key="f.key"
        @click="activeFilter = f.key"
        class="px-3 py-1 rounded-[14px] text-xs cursor-pointer border-[0.5px] border-border transition-all duration-150 font-[inherit] inline-flex items-center gap-1.5"
        :class="[
          activeFilter === f.key
            ? [f.activeBg, f.activeText, 'border-transparent', 'ring-1', 'ring-current/20']
            : 'bg-muted text-muted-foreground hover:bg-accent hover:text-accent-foreground'
        ]"
      >
        <span
          v-if="f.icon"
          class="inline-flex items-center justify-center rounded h-4 w-4"
          :class="f.iconColor"
        >
          <component :is="f.icon" class="h-2.5 w-2.5" />
        </span>
        {{ t(f.label) }}
      </button>
    </div>

    <!-- Model list (virtual scroll) -->
    <div v-if="filteredModels.length > 0" ref="scrollEl" class="flex-1 min-h-0 overflow-y-auto">
      <div :style="{ height: `${virtualizer.getTotalSize()}px`, position: 'relative' }">
        <div
          v-for="vItem in virtualizer.getVirtualItems()"
          :key="filteredModels[vItem.index]!.id"
          :data-index="vItem.index"
          :ref="(el) => virtualizer.measureElement(el as Element)"
          :style="{ position: 'absolute', top: 0, left: 0, width: '100%', transform: `translateY(${vItem.start}px)` }"
          class="pb-1.5"
        >
          <ModelCell
            :model="filteredModels[vItem.index]!"
            @download="handleDownload"
            @delete="handleDeleteRequest"
          />
        </div>
      </div>
    </div>

    <div v-else class="text-muted-foreground text-sm py-8 text-center">
      {{ t('modelManager.noModels') }}
    </div>

    <ConfirmDialog
      v-model:open="showDeleteConfirm"
      :title="t('modelManager.deleteConfirm')"
      :description="t('modelManager.deleteConfirmDesc', [deleteTarget?.label || ''])"
      :confirm-label="t('modelManager.delete')"
      @confirm="confirmDelete"
    />
  </div>
</template>
