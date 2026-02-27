<script setup lang="ts">
import { ref, computed, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore, type ASRModel, type EngineInfo } from '../stores/app'
import ModelCell from '../components/ModelCell.vue'
import ApiServerForm from '../components/ApiServerForm.vue'

const { t } = useI18n()
const store = useAppStore()

const selectedEngineId = ref<string | null>(null)
const showApiServerForm = ref(false)
const showDeleteConfirm = ref(false)
const deleteTarget = ref<ASRModel | null>(null)

const filteredModels = computed(() => {
  if (!selectedEngineId.value) return store.models
  return store.models.filter(m => m.engine_id === selectedEngineId.value)
})

const selectedEngineInfo = computed(() => {
  return store.engines.find(e => e.id === selectedEngineId.value)
})

function selectEngine(engine: EngineInfo) {
  selectedEngineId.value = engine.id
}

async function handleDownload(model: ASRModel) {
  await store.downloadModel(model.id)
}

async function handleSelect(model: ASRModel) {
  await store.selectModel(model.id)
}

function handleDeleteRequest(model: ASRModel) {
  deleteTarget.value = model
  showDeleteConfirm.value = true
}

async function confirmDelete() {
  if (deleteTarget.value) {
    await store.deleteModel(deleteTarget.value.id)
  }
  showDeleteConfirm.value = false
  deleteTarget.value = null
}

function cancelDelete() {
  showDeleteConfirm.value = false
  deleteTarget.value = null
}

onMounted(async () => {
  await store.fetchEngines()
  await store.fetchModels()
  if (store.engines.length > 0 && !selectedEngineId.value) {
    selectedEngineId.value = store.engines[0]?.id ?? null
  }
})
</script>

<template>
  <div class="flex h-screen">
    <!-- Engine sidebar -->
    <div class="w-48 border-r border-border bg-muted/30 overflow-y-auto flex-shrink-0">
      <div class="p-3">
        <h2 class="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-2">
          {{ t('modelManager.engines') }}
        </h2>
      </div>
      <div class="space-y-0.5 px-1">
        <button
          v-for="engine in store.engines"
          :key="engine.id"
          @click="selectEngine(engine)"
          class="w-full text-left px-3 py-2 rounded-md text-sm transition-colors"
          :class="selectedEngineId === engine.id
            ? 'bg-accent text-accent-foreground'
            : 'hover:bg-accent/50 text-foreground'"
        >
          <div class="flex items-center gap-2">
            <span
              class="w-2 h-2 rounded-full flex-shrink-0"
              :class="engine.available ? 'bg-green-500' : 'bg-gray-400'"
            />
            <div class="min-w-0">
              <div class="font-medium truncate">{{ engine.name }}</div>
              <div class="text-xs text-muted-foreground truncate">
                {{ engine.tool_name || (engine.available ? '' : t('modelManager.notInstalled')) }}
              </div>
            </div>
          </div>
        </button>
      </div>

      <!-- Add API Server button -->
      <div class="p-3 mt-2">
        <button
          @click="showApiServerForm = true"
          class="w-full text-sm px-3 py-1.5 rounded-md border border-border text-foreground hover:bg-accent transition-colors"
        >
          + {{ t('modelManager.addApiServer') }}
        </button>
      </div>
    </div>

    <!-- Model list -->
    <div class="flex-1 overflow-y-auto p-4">
      <h2 class="text-lg font-semibold mb-4">
        {{ selectedEngineInfo?.name || t('modelManager.models') }}
      </h2>

      <div class="space-y-2">
        <ModelCell
          v-for="model in filteredModels"
          :key="model.id"
          :model="model"
          :is-selected="model.id === store.selectedModelId"
          :is-downloading="model.id === store.downloadingModelId"
          :download-progress="model.id === store.downloadingModelId ? store.downloadProgress : 0"
          @select="handleSelect"
          @download="handleDownload"
          @delete="handleDeleteRequest"
        />
      </div>

      <div v-if="filteredModels.length === 0" class="text-muted-foreground text-sm py-8 text-center">
        {{ t('modelManager.notInstalled') }}
      </div>
    </div>

    <!-- Delete confirmation dialog -->
    <div v-if="showDeleteConfirm" class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
      <div class="bg-background border border-border rounded-lg p-6 max-w-sm mx-4 shadow-xl">
        <h3 class="text-lg font-semibold mb-2">{{ t('modelManager.deleteConfirm') }}</h3>
        <p class="text-sm text-muted-foreground mb-4">
          {{ t('modelManager.deleteConfirmDesc', [deleteTarget?.label || '']) }}
        </p>
        <div class="flex gap-2 justify-end">
          <button
            @click="cancelDelete"
            class="px-4 py-2 text-sm rounded-md border border-border hover:bg-accent transition-colors"
          >
            {{ t('modelManager.cancel') }}
          </button>
          <button
            @click="confirmDelete"
            class="px-4 py-2 text-sm rounded-md bg-destructive text-destructive-foreground hover:bg-destructive/90 transition-colors"
          >
            {{ t('modelManager.delete') }}
          </button>
        </div>
      </div>
    </div>

    <!-- API Server form dialog -->
    <ApiServerForm
      v-if="showApiServerForm"
      @close="showApiServerForm = false"
    />
  </div>
</template>
