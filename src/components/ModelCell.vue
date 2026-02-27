<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import type { ASRModel } from '../stores/app'

const { t } = useI18n()

const props = defineProps<{
  model: ASRModel
  isSelected: boolean
  isDownloading: boolean
  downloadProgress: number
}>()

const emit = defineEmits<{
  select: [model: ASRModel]
  download: [model: ASRModel]
  delete: [model: ASRModel]
}>()

const isDownloaded = computed(() => {
  const dt = props.model.download_type.type
  if (dt === 'RemoteAPI' || dt === 'System') return true
  return props.model.is_downloaded
})

const isRemoteAPI = computed(() => props.model.download_type.type === 'RemoteAPI')
</script>

<template>
  <div
    class="flex items-center gap-3 px-4 py-3 rounded-lg border border-border bg-card transition-colors hover:bg-accent/30"
    :class="{ 'ring-2 ring-primary/30': isSelected }"
  >
    <!-- Radio button (only clickable if downloaded) -->
    <button
      @click="isDownloaded ? emit('select', model) : null"
      :disabled="!isDownloaded"
      class="flex-shrink-0"
    >
      <div
        class="w-4 h-4 rounded-full border-2 flex items-center justify-center transition-colors"
        :class="isSelected
          ? 'border-primary bg-primary'
          : isDownloaded
            ? 'border-muted-foreground hover:border-primary'
            : 'border-muted opacity-50'"
      >
        <div v-if="isSelected" class="w-2 h-2 rounded-full bg-primary-foreground" />
      </div>
    </button>

    <!-- Model info -->
    <div class="flex-1 min-w-0">
      <div class="font-medium text-sm">{{ model.label }}</div>
      <div v-if="model.size" class="text-xs text-muted-foreground">{{ model.size }}</div>
    </div>

    <!-- Status / Actions -->
    <div class="flex items-center gap-2 flex-shrink-0">
      <!-- Downloading -->
      <template v-if="isDownloading">
        <div class="w-24 h-1.5 bg-muted rounded-full overflow-hidden">
          <div
            class="h-full bg-primary rounded-full transition-all duration-300"
            :style="{ width: `${downloadProgress * 100}%` }"
          />
        </div>
        <span class="text-xs text-muted-foreground w-10 text-right">
          {{ Math.round(downloadProgress * 100) }}%
        </span>
      </template>

      <!-- Downloaded -->
      <template v-else-if="isDownloaded && !isRemoteAPI">
        <span class="text-xs text-green-500 font-medium">{{ t('modelManager.downloaded') }}</span>
        <button
          @click="emit('delete', model)"
          class="text-muted-foreground hover:text-destructive transition-colors p-1"
          :title="t('modelManager.delete')"
        >
          <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
          </svg>
        </button>
      </template>

      <!-- Remote API -->
      <template v-else-if="isRemoteAPI">
        <span class="text-xs text-blue-500 font-medium">API</span>
        <button
          @click="emit('delete', model)"
          class="text-muted-foreground hover:text-destructive transition-colors p-1"
          :title="t('modelManager.delete')"
        >
          <svg xmlns="http://www.w3.org/2000/svg" class="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 6h18"/><path d="M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6"/><path d="M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2"/>
          </svg>
        </button>
      </template>

      <!-- Not downloaded -->
      <template v-else>
        <button
          @click="emit('download', model)"
          class="px-3 py-1 text-xs font-medium rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
        >
          {{ t('modelManager.download') }}
        </button>
      </template>
    </div>
  </div>
</template>
