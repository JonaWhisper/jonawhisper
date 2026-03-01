<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/settings'
import { useEnginesStore } from '@/stores/engines'
import { useDownloadStore } from '@/stores/downloads'
import { isModelAvailable } from '@/stores/types'
import type { ASRModel } from '@/stores/types'
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
import { ChevronRight, Pause, Play, X, Loader2 } from 'lucide-vue-next'
import ShortcutCapture from '@/components/ShortcutCapture.vue'
import SegmentedToggle from '@/components/SegmentedToggle.vue'
import BenchmarkBadges from '@/components/BenchmarkBadges.vue'
import { formatSize, formatSpeed } from '@/utils/format'

const { t } = useI18n()
const settings = useSettingsStore()
const engines = useEnginesStore()
const downloads = useDownloadStore()
const emit = defineEmits<{ start: []; back: [] }>()

const showAllModels = ref(false)

// -- Hotkey --
async function onHotkeyChange(value: string) {
  await settings.setSetting('hotkey', value)
}

// -- Recording mode --
async function onRecordingModeChange(mode: string) {
  await settings.setSetting('recording_mode', mode)
}

// -- Models --
const availableEngines = computed(() => engines.engines.filter(e => e.available && e.category === 'asr'))

const recommendedModels = computed(() => {
  const result = engines.models.filter(m => m.recommended)
  // Include the currently selected model if not already in the list
  if (settings.selectedModelId && !result.find(m => m.id === settings.selectedModelId)) {
    const selected = engines.models.find(m => m.id === settings.selectedModelId)
    if (selected) result.unshift(selected)
  }
  return result
})

const recommendedByEngine = computed(() => {
  const groups: { engine: string; engineName: string; models: ASRModel[] }[] = []
  for (const engine of availableEngines.value) {
    const models = recommendedModels.value.filter(m => m.engine_id === engine.id)
    if (models.length > 0) {
      groups.push({ engine: engine.id, engineName: engine.name, models })
    }
  }
  return groups
})

const allModelsByEngine = computed(() => {
  const groups: { engine: string; engineName: string; models: ASRModel[] }[] = []
  for (const engine of availableEngines.value) {
    const engineModels = engines.models.filter(m => m.engine_id === engine.id)
    if (engineModels.length > 0) {
      groups.push({ engine: engine.id, engineName: engine.name, models: engineModels })
    }
  }
  return groups
})

const isModelDownloaded = isModelAvailable


async function handleDownload(model: ASRModel) {
  const success = await downloads.downloadModel(model.id)
  if (success) {
    await settings.selectModel(model.id)
  }
}

async function handleSelectModel(model: ASRModel) {
  if (!isModelDownloaded(model)) return
  // Check language compatibility
  const engine = engines.engines.find(e => e.id === model.engine_id)
  if (engine && settings.selectedLanguage !== 'auto') {
    if (!engine.supported_language_codes.includes(settings.selectedLanguage)) {
      await settings.selectLanguageAction('auto')
    }
  }
  await settings.selectModel(model.id)
}

// -- Transcription language --
const availableLanguages = computed(() => {
  const model = engines.models.find(m => m.id === settings.selectedModelId)
  if (!model) return engines.languages
  const engine = engines.engines.find(e => e.id === model.engine_id)
  if (!engine) return engines.languages
  return engines.languages.filter(l => engine.supported_language_codes.includes(l.code))
})

async function onLanguageChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  await settings.selectLanguageAction(value)
}

// Refresh models when language changes (recommended flags depend on it)
watch(() => settings.selectedLanguage, () => {
  engines.fetchModels()
})

// Reset language if no longer supported when model changes
watch(() => settings.selectedModelId, () => {
  if (settings.selectedLanguage === 'auto') return
  const langs = availableLanguages.value
  if (!langs.find(l => l.code === settings.selectedLanguage)) {
    settings.selectLanguageAction('auto')
  }
})

// -- Can start --
const canStart = computed(() => {
  const model = engines.models.find(m => m.id === settings.selectedModelId)
  return model ? isModelDownloaded(model) : false
})
</script>

<template>
  <div class="flex flex-col h-full select-none">
    <!-- Header -->
    <div class="text-center px-5 pt-2 pb-3">
      <h1 class="text-lg font-bold">{{ t('setup.step2.title') }}</h1>
      <p class="text-xs text-muted-foreground mt-0.5">{{ t('setup.step2.subtitle') }}</p>
    </div>

    <!-- Two-column content -->
    <div class="flex-1 flex gap-5 px-5 min-h-0">
      <!-- Left column: quick settings -->
      <div class="w-[240px] shrink-0 space-y-4 overflow-y-auto">
        <!-- Hotkey -->
        <div class="space-y-1">
          <Label class="text-sm font-medium">{{ t('setup.step2.hotkey') }}</Label>
          <ShortcutCapture
            :model-value="settings.hotkey"
            @update:model-value="onHotkeyChange"
          />
        </div>

        <!-- Recording mode -->
        <div class="space-y-1">
          <Label class="text-sm font-medium">{{ t('setup.step2.recordingMode') }}</Label>
          <SegmentedToggle
            :model-value="settings.recordingMode"
            :options="[
              { value: 'push_to_talk', label: t('setup.step2.pushToTalk') },
              { value: 'toggle', label: t('setup.step2.toggle') },
            ]"
            block
            @update:model-value="onRecordingModeChange"
          />
        </div>

        <!-- Transcription language -->
        <div class="space-y-1">
          <Label class="text-sm font-medium">{{ t('setup.step2.transcriptionLanguage') }}</Label>
          <Select :model-value="settings.selectedLanguage" @update:model-value="onLanguageChange">
            <SelectTrigger class="w-full h-9 text-sm">
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
          <p class="text-xs text-muted-foreground">{{ t('setup.step2.modelDesc') }}</p>
        </div>

        <div class="flex-1 overflow-y-auto space-y-2">
          <!-- Models grouped by engine (recommended or all) -->
          <template v-for="group in (showAllModels ? allModelsByEngine : recommendedByEngine)" :key="group.engine">
            <div class="mb-2">
              <div class="text-xs font-semibold text-muted-foreground uppercase tracking-wider mb-1">
                {{ group.engineName }}
              </div>
              <div class="space-y-1">
                <div
                  v-for="model in group.models"
                  :key="model.id"
                  class="flex items-center gap-2 px-3 py-2 rounded-lg border cursor-pointer transition-colors hover:bg-accent/30"
                  :class="model.id === settings.selectedModelId ? 'bg-primary/10 border-primary/30' : 'bg-card border-border'"
                  @click="handleSelectModel(model)"
                >
                  <div
                    class="w-3.5 h-3.5 rounded-full border-2 flex items-center justify-center flex-shrink-0"
                    :class="model.id === settings.selectedModelId
                      ? 'border-primary bg-primary'
                      : isModelDownloaded(model)
                        ? 'border-muted-foreground'
                        : 'border-muted opacity-50'"
                  >
                    <div v-if="model.id === settings.selectedModelId" class="w-1.5 h-1.5 rounded-full bg-primary-foreground" />
                  </div>
                  <div class="flex-1 min-w-0">
                    <div class="flex items-center gap-1.5">
                      <span class="text-sm font-medium truncate">{{ model.label }}</span>
                      <span v-if="model.size > 0" class="text-xs text-muted-foreground shrink-0">{{ formatSize(model.size) }}</span>
                    </div>
                    <BenchmarkBadges v-if="model.wer != null || model.rtf != null || model.params != null || model.ram != null || (model.lang_codes && model.lang_codes.length > 0)" :wer="model.wer" :rtf="model.rtf" :params="model.params" :ram="model.ram" :lang-codes="model.lang_codes" compact class="mt-0.5" />
                  </div>
                  <div class="flex items-center gap-1.5 flex-shrink-0" @click.stop>
                    <!-- Downloading -->
                    <template v-if="downloads.activeDownloads[model.id]">
                      <div class="w-16">
                        <Progress :model-value="(downloads.activeDownloads[model.id]?.progress ?? 0) * 100" />
                        <div class="text-[9px] text-muted-foreground mt-0.5">
                          {{ formatSpeed(downloads.activeDownloads[model.id]!.speed) }}
                        </div>
                      </div>
                      <template v-if="downloads.activeDownloads[model.id]?.stopping">
                        <Loader2 class="w-3.5 h-3.5 animate-spin text-muted-foreground" />
                      </template>
                      <template v-else>
                        <Button variant="ghost" size="icon-sm" @click="downloads.pauseDownload(model.id)" :title="t('modelManager.pause')">
                          <Pause class="w-3.5 h-3.5" />
                        </Button>
                        <Button variant="ghost" size="icon-sm" @click="downloads.cancelDownload(model.id)" :title="t('modelManager.cancel')">
                          <X class="w-3.5 h-3.5" />
                        </Button>
                      </template>
                    </template>
                    <!-- Paused (partial exists) -->
                    <template v-else-if="model.partial_progress != null && model.partial_progress > 0">
                      <div class="w-16">
                        <Progress :model-value="(model.partial_progress ?? 0) * 100" />
                        <div v-if="model.size > 0" class="text-[9px] text-muted-foreground mt-0.5">
                          {{ formatSize(Math.round((model.partial_progress ?? 0) * model.size)) }} / {{ formatSize(model.size) }}
                        </div>
                      </div>
                      <Button variant="ghost" size="icon-sm" @click="handleDownload(model)" :title="t('modelManager.resume')">
                        <Play class="w-3.5 h-3.5" />
                      </Button>
                      <Button variant="ghost" size="icon-sm" @click="downloads.cancelDownload(model.id)" :title="t('modelManager.cancel')">
                        <X class="w-3.5 h-3.5" />
                      </Button>
                    </template>
                    <Badge v-else-if="isModelDownloaded(model)" variant="secondary" class="bg-green-500/10 text-green-500 border-transparent text-xs">
                      {{ t('modelManager.downloaded') }}
                    </Badge>
                    <Button v-else size="sm" @click="handleDownload(model)">
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
    <div class="px-5 pt-3 pb-4 border-t border-border mt-2">
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
