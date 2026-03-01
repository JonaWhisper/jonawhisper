<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useEnginesStore } from '@/stores/engines'
import { useDownloadStore } from '@/stores/downloads'
import type { ASRModel, EngineInfo } from '@/stores/types'
import ModelCell from '@/components/ModelCell.vue'
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

const { t } = useI18n()
const engines = useEnginesStore()
const downloads = useDownloadStore()

const selectedEngineId = ref<string | null>(null)
const showDeleteConfirm = ref(false)
const deleteTarget = ref<ASRModel | null>(null)

const filteredModels = computed(() => {
  if (!selectedEngineId.value) return engines.models
  return engines.models.filter(m => m.engine_id === selectedEngineId.value)
})

const selectedEngineInfo = computed(() => {
  return engines.engines.find(e => e.id === selectedEngineId.value)
})

function selectEngine(engine: EngineInfo) {
  selectedEngineId.value = engine.id
}

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

onMounted(async () => {
  getCurrentWindow().setTitle(t('window.modelManager'))
  await Promise.all([engines.fetchEngines(), engines.fetchModels()])
  if (engines.engines.length > 0 && !selectedEngineId.value) {
    // Default to first ASR engine
    const firstAsr = engines.asrEngines[0]
    selectedEngineId.value = firstAsr?.id ?? engines.engines[0]?.id ?? null
  }
})
</script>

<template>
  <div class="flex h-full">
    <!-- Engine sidebar -->
    <div class="w-48 border-r border-border bg-muted/30 overflow-y-auto flex-shrink-0">
      <div class="px-3 pt-3 pb-1">
        <h2 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
          {{ t('modelManager.engines') }}
        </h2>
      </div>
      <div class="space-y-1 px-1">
        <button
          v-for="engine in engines.asrEngines"
          :key="engine.id"
          @click="selectEngine(engine)"
          class="w-full text-left px-3 py-2 rounded-md text-sm transition-colors"
          :class="selectedEngineId === engine.id
            ? 'bg-accent text-accent-foreground'
            : 'hover:bg-accent/50 text-foreground'"
        >
          <span class="font-medium truncate">{{ engine.name }}</span>
        </button>
      </div>
      <template v-if="engines.punctuationEngines.length > 0">
        <div class="px-3 pt-4 pb-1">
          <h2 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
            {{ t('modelManager.punctuation') }}
          </h2>
        </div>
        <div class="space-y-1 px-1">
          <button
            v-for="engine in engines.punctuationEngines"
            :key="engine.id"
            @click="selectEngine(engine)"
            class="w-full text-left px-3 py-2 rounded-md text-sm transition-colors"
            :class="selectedEngineId === engine.id
              ? 'bg-accent text-accent-foreground'
              : 'hover:bg-accent/50 text-foreground'"
          >
            <span class="font-medium truncate">{{ engine.name }}</span>
          </button>
        </div>
      </template>
      <template v-if="engines.llmEngines.length > 0">
        <div class="px-3 pt-4 pb-1">
          <h2 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
            {{ t('modelManager.postProcessing') }}
          </h2>
        </div>
        <div class="space-y-1 px-1">
          <button
            v-for="engine in engines.llmEngines"
            :key="engine.id"
            @click="selectEngine(engine)"
            class="w-full text-left px-3 py-2 rounded-md text-sm transition-colors"
            :class="selectedEngineId === engine.id
              ? 'bg-accent text-accent-foreground'
              : 'hover:bg-accent/50 text-foreground'"
          >
            <span class="font-medium truncate">{{ engine.name }}</span>
          </button>
        </div>
      </template>
    </div>

    <!-- Main content -->
    <div class="flex-1 flex flex-col min-w-0 overflow-hidden">
      <!-- Scrollable model list -->
      <div class="flex-1 overflow-y-auto p-5">
        <!-- Engine header with install hint -->
        <div class="mb-4">
          <h2 class="text-lg font-semibold">
            {{ selectedEngineInfo?.name || t('modelManager.models') }}
          </h2>
          <p v-if="selectedEngineInfo?.description" class="text-sm text-muted-foreground mt-0.5">
            {{ selectedEngineInfo.description }}
          </p>
        </div>

        <div class="space-y-2">
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
      </div>
    </div>

    <!-- Delete confirmation dialog -->
    <AlertDialog :open="showDeleteConfirm" @update:open="showDeleteConfirm = $event">
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{{ t('modelManager.deleteConfirm') }}</AlertDialogTitle>
          <AlertDialogDescription>
            {{ t('modelManager.deleteConfirmDesc', [deleteTarget?.label || '']) }}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel @click="showDeleteConfirm = false">{{ t('modelManager.cancel') }}</AlertDialogCancel>
          <AlertDialogAction @click="confirmDelete" class="bg-destructive text-destructive-foreground hover:bg-destructive/90">
            {{ t('modelManager.delete') }}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  </div>
</template>
