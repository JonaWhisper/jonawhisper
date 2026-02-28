<script setup lang="ts">
import { ref, computed, watch, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore, type ASRModel, type EngineInfo } from '@/stores/app'
import ModelCell from '@/components/ModelCell.vue'
import ApiServerForm from '@/components/ApiServerForm.vue'
import { Button } from '@/components/ui/button'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
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
const store = useAppStore()

const selectedEngineId = ref<string | null>(null)
const showApiServerForm = ref(false)
const showDeleteConfirm = ref(false)
const deleteTarget = ref<ASRModel | null>(null)
const languageWarning = ref<string | null>(null)

const filteredModels = computed(() => {
  if (!selectedEngineId.value) return store.models
  return store.models.filter(m => m.engine_id === selectedEngineId.value)
})

const selectedEngineInfo = computed(() => {
  return store.engines.find(e => e.id === selectedEngineId.value)
})

// Languages filtered to the currently selected engine
const availableLanguages = computed(() => {
  const engine = store.engines.find(e => {
    const model = store.models.find(m => m.id === store.selectedModelId)
    return model && e.id === model.engine_id
  })
  if (!engine) return store.languages
  return store.languages.filter(l => engine.supported_language_codes.includes(l.code))
})

function selectEngine(engine: EngineInfo) {
  selectedEngineId.value = engine.id
}

async function handleLanguageChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value === 'string') {
    await store.selectLanguageAction(value)
  }
}

async function handleDownload(model: ASRModel) {
  await store.downloadModel(model.id)
}

async function handleSelect(model: ASRModel) {
  // Check language compatibility before selecting
  const engine = store.engines.find(e => e.id === model.engine_id)
  if (engine && store.selectedLanguage !== 'auto') {
    if (!engine.supported_language_codes.includes(store.selectedLanguage)) {
      const langLabel = store.languages.find(l => l.code === store.selectedLanguage)?.label || store.selectedLanguage
      languageWarning.value = t('modelManager.languageWarning', [langLabel])
      await store.selectLanguageAction('auto')
    }
  }
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

// Auto-dismiss language warning
watch(languageWarning, (val) => {
  if (val) {
    setTimeout(() => { languageWarning.value = null }, 4000)
  }
})

onMounted(async () => {
  await Promise.all([store.fetchEngines(), store.fetchModels(), store.fetchLanguages()])
  if (store.engines.length > 0 && !selectedEngineId.value) {
    selectedEngineId.value = store.engines[0]?.id ?? null
  }
})
</script>

<template>
  <div class="flex h-full">
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

    </div>

    <!-- Main content -->
    <div class="flex-1 flex flex-col min-w-0 overflow-hidden">
      <!-- Fixed toolbar: language + add server -->
      <div class="flex items-center gap-3 px-4 py-2.5 border-b border-border bg-background flex-shrink-0">
        <label class="text-xs font-medium text-muted-foreground whitespace-nowrap">
          {{ t('modelManager.language') }}
        </label>
        <Select :model-value="store.selectedLanguage" @update:model-value="handleLanguageChange">
          <SelectTrigger class="w-40">
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            <SelectItem
              v-for="lang in availableLanguages"
              :key="lang.code"
              :value="lang.code"
            >
              {{ lang.label }}
            </SelectItem>
          </SelectContent>
        </Select>
        <div class="flex-1" />
        <Button variant="outline" size="sm" @click="showApiServerForm = true">
          + {{ t('modelManager.addApiServer') }}
        </Button>
      </div>

      <!-- Scrollable model list -->
      <div class="flex-1 overflow-y-auto p-4">
        <!-- Language warning -->
        <div v-if="languageWarning" class="mb-4 px-3 py-2 rounded-md bg-yellow-500/10 border border-yellow-500/30 text-yellow-600 dark:text-yellow-400 text-sm">
          {{ languageWarning }}
        </div>

        <!-- Engine header with install hint (always shown when hint exists) -->
        <div class="mb-4">
          <h2 class="text-lg font-semibold">
            {{ selectedEngineInfo?.name || t('modelManager.models') }}
          </h2>
          <p v-if="selectedEngineInfo?.description" class="text-sm text-muted-foreground mt-0.5">
            {{ selectedEngineInfo.description }}
          </p>
          <div v-if="selectedEngineInfo?.install_hint" class="mt-1 text-sm text-muted-foreground">
            {{ t('modelManager.installWith') }}
            <code class="px-1.5 py-0.5 rounded bg-muted text-xs font-mono">{{ selectedEngineInfo.install_hint }}</code>
          </div>
        </div>

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

    <!-- API Server form dialog -->
    <ApiServerForm
      v-if="showApiServerForm"
      @close="showApiServerForm = false"
    />
  </div>
</template>
