<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { useAppStore, type ASRModel } from '@/stores/app'
import { i18n } from '@/main'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { Label } from '@/components/ui/label'
import { Progress } from '@/components/ui/progress'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { ChevronRight } from 'lucide-vue-next'
import ShortcutCapture from '@/components/ShortcutCapture.vue'
import BenchmarkBadges from '@/components/BenchmarkBadges.vue'

const { t } = useI18n()
const store = useAppStore()
const emit = defineEmits<{ start: []; back: [] }>()

const showAllModels = ref(false)

// -- Locale --
const localeOptions = [
  { value: 'auto', label: 'settings.locale.auto' },
  { value: 'fr', label: 'settings.locale.fr' },
  { value: 'en', label: 'settings.locale.en' },
]

async function onLocaleChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await store.setSetting('app_locale', value)
  // Rust handles tray labels; resolve effective locale for frontend
  const locale = await invoke<string>('get_system_locale')
  i18n.global.locale.value = locale as 'fr' | 'en'
}

// -- Hotkey --
async function onHotkeyChange(value: string) {
  await store.setSetting('hotkey', value)
}

// -- Recording mode --
async function onRecordingModeChange(mode: string) {
  await store.setSetting('recording_mode', mode)
}

// -- Models --
const RECOMMENDED: Record<string, string> = {
  'whisper': 'whisper:large-v3-turbo',
  'faster-whisper': 'faster-whisper:large-v3-turbo',
  'mlx-whisper': 'mlx-whisper:large-v3-turbo',
  'moonshine': 'moonshine:base',
  'vosk': navigator.language.startsWith('fr') ? 'vosk:fr-small' : 'vosk:en-small',
}

const availableEngines = computed(() => store.engines.filter(e => e.available))

const recommendedModels = computed(() => {
  const result: ASRModel[] = []
  const addedIds = new Set<string>()
  for (const engine of availableEngines.value) {
    const recId = RECOMMENDED[engine.id]
    if (recId) {
      const model = store.models.find(m => m.id === recId)
      if (model) { result.push(model); addedIds.add(model.id) }
    }
  }
  // Include the currently selected model if not already in the list
  if (store.selectedModelId && !addedIds.has(store.selectedModelId)) {
    const selected = store.models.find(m => m.id === store.selectedModelId)
    if (selected) result.unshift(selected)
  }
  return result
})

const allModelsByEngine = computed(() => {
  const groups: { engine: string; engineName: string; models: ASRModel[] }[] = []
  for (const engine of availableEngines.value) {
    const engineModels = store.models.filter(m => m.engine_id === engine.id)
    if (engineModels.length > 0) {
      groups.push({ engine: engine.id, engineName: engine.name, models: engineModels })
    }
  }
  return groups
})

function isModelDownloaded(model: ASRModel): boolean {
  const dt = model.download_type.type
  if (dt === 'RemoteAPI' || dt === 'System') return true
  return !!model.is_downloaded
}

function formatSize(bytes: number): string {
  if (bytes <= 0) return ''
  if (bytes >= 1_000_000_000) return t('size.gb', [+(bytes / 1_000_000_000).toFixed(1)])
  return t('size.mb', [Math.round(bytes / 1_000_000)])
}

async function handleDownload(model: ASRModel) {
  const success = await store.downloadModel(model.id)
  if (success) {
    await store.selectModel(model.id)
  }
}

async function handleSelectModel(model: ASRModel) {
  if (!isModelDownloaded(model)) return
  // Check language compatibility
  const engine = store.engines.find(e => e.id === model.engine_id)
  if (engine && store.selectedLanguage !== 'auto') {
    if (!engine.supported_language_codes.includes(store.selectedLanguage)) {
      await store.selectLanguageAction('auto')
    }
  }
  await store.selectModel(model.id)
}

// -- Transcription language --
const availableLanguages = computed(() => {
  const model = store.models.find(m => m.id === store.selectedModelId)
  if (!model) return store.languages
  const engine = store.engines.find(e => e.id === model.engine_id)
  if (!engine) return store.languages
  return store.languages.filter(l => engine.supported_language_codes.includes(l.code))
})

async function onLanguageChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await store.selectLanguageAction(value)
}

// Reset language if no longer supported when model changes
watch(() => store.selectedModelId, () => {
  if (store.selectedLanguage === 'auto') return
  const langs = availableLanguages.value
  if (!langs.find(l => l.code === store.selectedLanguage)) {
    store.selectLanguageAction('auto')
  }
})

// -- Can start --
const canStart = computed(() => {
  const model = store.models.find(m => m.id === store.selectedModelId)
  return model ? isModelDownloaded(model) : false
})
</script>

<template>
  <div class="flex flex-col h-full select-none">
    <!-- Header -->
    <div class="text-center px-5 pt-4 pb-3">
      <h1 class="text-lg font-bold">{{ t('setup.step2.title') }}</h1>
      <p class="text-xs text-muted-foreground mt-0.5">{{ t('setup.step2.subtitle') }}</p>
    </div>

    <!-- Two-column content -->
    <div class="flex-1 flex gap-5 px-5 min-h-0">
      <!-- Left column: quick settings -->
      <div class="w-[240px] shrink-0 space-y-3.5 overflow-y-auto">
        <!-- UI Language -->
        <div class="space-y-1">
          <Label class="text-sm font-medium">{{ t('setup.step2.uiLanguage') }}</Label>
          <Select :model-value="store.appLocale" @update:model-value="onLocaleChange">
            <SelectTrigger class="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="opt in localeOptions" :key="opt.value" :value="opt.value">
                {{ t(opt.label) }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>

        <!-- Hotkey -->
        <div class="space-y-1">
          <Label class="text-sm font-medium">{{ t('setup.step2.hotkey') }}</Label>
          <ShortcutCapture
            :model-value="store.hotkey"
            @update:model-value="onHotkeyChange"
          />
        </div>

        <!-- Recording mode -->
        <div class="space-y-1">
          <Label class="text-sm font-medium">{{ t('setup.step2.recordingMode') }}</Label>
          <div class="inline-flex rounded-md border border-border overflow-hidden w-full">
            <button
              class="flex-1 px-3 py-1.5 text-sm transition-colors"
              :class="store.recordingMode === 'push_to_talk'
                ? 'bg-accent text-accent-foreground font-medium'
                : 'hover:bg-accent/50 text-muted-foreground'"
              @click="onRecordingModeChange('push_to_talk')"
            >
              {{ t('setup.step2.pushToTalk') }}
            </button>
            <button
              class="flex-1 px-3 py-1.5 text-sm border-l border-border transition-colors"
              :class="store.recordingMode === 'toggle'
                ? 'bg-accent text-accent-foreground font-medium'
                : 'hover:bg-accent/50 text-muted-foreground'"
              @click="onRecordingModeChange('toggle')"
            >
              {{ t('setup.step2.toggle') }}
            </button>
          </div>
        </div>

        <!-- Transcription language -->
        <div class="space-y-1">
          <Label class="text-sm font-medium">{{ t('setup.step2.transcriptionLanguage') }}</Label>
          <Select :model-value="store.selectedLanguage" @update:model-value="onLanguageChange">
            <SelectTrigger class="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem v-for="lang in availableLanguages" :key="lang.code" :value="lang.code">
                {{ lang.label }}
              </SelectItem>
            </SelectContent>
          </Select>
        </div>
      </div>

      <!-- Right column: model picker -->
      <div class="flex-1 flex flex-col min-w-0 min-h-0">
        <div class="mb-1.5">
          <Label class="text-sm font-medium">{{ t('setup.step2.model') }}</Label>
          <p class="text-[11px] text-muted-foreground">{{ t('setup.step2.modelDesc') }}</p>
        </div>

        <div class="flex-1 overflow-y-auto space-y-1.5">
          <!-- Recommended models (flat list) -->
          <template v-if="!showAllModels">
            <div
              v-for="model in recommendedModels"
              :key="model.id"
              class="flex items-center gap-2 px-2.5 py-1.5 rounded-lg border cursor-pointer transition-colors hover:bg-accent/30"
              :class="model.id === store.selectedModelId ? 'bg-primary/10 border-primary/30' : 'bg-card border-border'"
              @click="handleSelectModel(model)"
            >
              <div
                class="w-3.5 h-3.5 rounded-full border-2 flex items-center justify-center flex-shrink-0"
                :class="model.id === store.selectedModelId
                  ? 'border-primary bg-primary'
                  : isModelDownloaded(model)
                    ? 'border-muted-foreground'
                    : 'border-muted opacity-50'"
              >
                <div v-if="model.id === store.selectedModelId" class="w-1.5 h-1.5 rounded-full bg-primary-foreground" />
              </div>
              <div class="flex-1 min-w-0">
                <div class="text-sm font-medium truncate">{{ model.label }}</div>
                <div class="flex items-center gap-1 flex-wrap text-[11px] text-muted-foreground">
                  <span v-if="model.size > 0">{{ formatSize(model.size) }}</span>
                  <template v-if="model.wer != null || model.rtf != null">
                    <span v-if="model.size > 0" class="opacity-40">&middot;</span>
                    <BenchmarkBadges :wer="model.wer" :rtf="model.rtf" compact />
                  </template>
                </div>
              </div>
              <div class="flex items-center gap-1.5 flex-shrink-0" @click.stop>
                <template v-if="store.downloadingModelId === model.id">
                  <Progress :model-value="store.downloadProgress * 100" class="w-16" />
                  <span class="text-[11px] text-muted-foreground w-8 text-right">
                    {{ Math.round(store.downloadProgress * 100) }}%
                  </span>
                </template>
                <Badge v-else-if="isModelDownloaded(model)" variant="secondary" class="bg-green-500/10 text-green-500 border-transparent text-[11px]">
                  {{ t('modelManager.downloaded') }}
                </Badge>
                <Button v-else size="sm" class="h-6 text-xs px-2" @click="handleDownload(model)">
                  {{ t('modelManager.download') }}
                </Button>
              </div>
            </div>
          </template>

          <!-- All models (grouped by engine) -->
          <template v-else>
            <div v-for="group in allModelsByEngine" :key="group.engine" class="mb-2">
              <div class="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-1">
                {{ group.engineName }}
              </div>
              <div class="space-y-1">
                <div
                  v-for="model in group.models"
                  :key="model.id"
                  class="flex items-center gap-2 px-2.5 py-1.5 rounded-lg border cursor-pointer transition-colors hover:bg-accent/30"
                  :class="model.id === store.selectedModelId ? 'bg-primary/10 border-primary/30' : 'bg-card border-border'"
                  @click="handleSelectModel(model)"
                >
                  <div
                    class="w-3.5 h-3.5 rounded-full border-2 flex items-center justify-center flex-shrink-0"
                    :class="model.id === store.selectedModelId
                      ? 'border-primary bg-primary'
                      : isModelDownloaded(model)
                        ? 'border-muted-foreground'
                        : 'border-muted opacity-50'"
                  >
                    <div v-if="model.id === store.selectedModelId" class="w-1.5 h-1.5 rounded-full bg-primary-foreground" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <div class="text-sm font-medium truncate">{{ model.label }}</div>
                    <div class="flex items-center gap-1 flex-wrap text-[11px] text-muted-foreground">
                      <span v-if="model.size > 0">{{ formatSize(model.size) }}</span>
                      <template v-if="model.wer != null || model.rtf != null">
                        <span v-if="model.size > 0" class="opacity-40">&middot;</span>
                        <BenchmarkBadges :wer="model.wer" :rtf="model.rtf" compact />
                      </template>
                    </div>
                  </div>
                  <div class="flex items-center gap-1.5 flex-shrink-0" @click.stop>
                    <template v-if="store.downloadingModelId === model.id">
                      <Progress :model-value="store.downloadProgress * 100" class="w-16" />
                      <span class="text-[11px] text-muted-foreground w-8 text-right">
                        {{ Math.round(store.downloadProgress * 100) }}%
                      </span>
                    </template>
                    <Badge v-else-if="isModelDownloaded(model)" variant="secondary" class="bg-green-500/10 text-green-500 border-transparent text-[11px]">
                      {{ t('modelManager.downloaded') }}
                    </Badge>
                    <Button v-else size="sm" class="h-6 text-xs px-2" @click="handleDownload(model)">
                      {{ t('modelManager.download') }}
                    </Button>
                  </div>
                </div>
              </div>
            </div>
          </template>
        </div>

        <!-- Toggle all models -->
        <button
          class="flex items-center gap-1 text-xs text-primary hover:underline mt-1.5 shrink-0"
          @click="showAllModels = !showAllModels"
        >
          <ChevronRight class="w-3 h-3 transition-transform" :class="{ 'rotate-90': showAllModels }" />
          {{ showAllModels ? t('setup.step2.hideAllModels') : t('setup.step2.showAllModels') }}
        </button>
      </div>
    </div>

    <!-- Fixed bottom -->
    <div class="px-5 pt-3 pb-5 border-t border-border mt-3">
      <Button class="w-full" :disabled="!canStart" @click="emit('start')">
        {{ t('setup.step2.start') }}
      </Button>
      <button
        class="w-full mt-2 text-xs text-muted-foreground hover:text-foreground transition-colors text-center"
        @click="emit('back')"
      >
        {{ t('setup.step2.back') }}
      </button>
    </div>
  </div>
</template>
