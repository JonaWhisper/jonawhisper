<script setup lang="ts">
import { ref, computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEnginesStore } from '@/stores/engines'
import { useDownloadStore } from '@/stores/downloads'
import type { ASRModel } from '@/stores/types'
import ModelCell from '@/components/ModelCell.vue'
import ConfirmDialog from '@/components/ConfirmDialog.vue'

const { t } = useI18n()
const engines = useEnginesStore()
const downloads = useDownloadStore()

type FilterKey = 'all' | 'asr' | 'punctuation' | 'correction' | 'llm'
const activeFilter = ref<FilterKey>('all')

const filters: { key: FilterKey; label: string }[] = [
  { key: 'all', label: 'models.filter.all' },
  { key: 'asr', label: 'models.filter.asr' },
  { key: 'punctuation', label: 'models.filter.punctuation' },
  { key: 'correction', label: 'models.filter.correction' },
  { key: 'llm', label: 'models.filter.llm' },
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
</script>

<template>
  <div>
    <div class="section-title">{{ t('panel.models') }}</div>

    <!-- Filter chips -->
    <div class="flex flex-wrap gap-1 mb-3.5">
      <button
        v-for="f in filters"
        :key="f.key"
        @click="activeFilter = f.key"
        class="wf-filter-chip"
        :class="{ active: activeFilter === f.key }"
      >
        {{ t(f.label) }}
      </button>
    </div>

    <!-- Model list -->
    <div class="space-y-1.5">
      <ModelCell
        v-for="model in filteredModels"
        :key="model.id"
        :model="model"
        @download="handleDownload"
        @delete="handleDeleteRequest"
      />
    </div>

    <div v-if="filteredModels.length === 0" class="text-muted-foreground text-sm py-8 text-center">
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
