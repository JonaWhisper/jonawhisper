<script setup lang="ts">
import { ref, defineAsyncComponent, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '@/stores/app'
import { Clock, Package, AudioLines, Sparkles, Keyboard, Mic, Cloud, Shield, Settings2, BookOpen } from 'lucide-vue-next'

// Lazy-load sections — only the active one is loaded
const RecentsSection = defineAsyncComponent(() => import('@/sections/RecentsSection.vue'))
const ModelsSection = defineAsyncComponent(() => import('@/sections/ModelsSection.vue'))
const TranscriptionSection = defineAsyncComponent(() => import('@/sections/TranscriptionSection.vue'))
const ProcessingSection = defineAsyncComponent(() => import('@/sections/ProcessingSection.vue'))
const ShortcutsSection = defineAsyncComponent(() => import('@/sections/ShortcutsSection.vue'))
const MicrophoneSection = defineAsyncComponent(() => import('@/sections/MicrophoneSection.vue'))
const ProvidersSection = defineAsyncComponent(() => import('@/sections/ProvidersSection.vue'))
const PermissionsSection = defineAsyncComponent(() => import('@/sections/PermissionsSection.vue'))
const GeneralSection = defineAsyncComponent(() => import('@/sections/GeneralSection.vue'))
const DictionarySection = defineAsyncComponent(() => import('@/sections/DictionarySection.vue'))

const { t } = useI18n()
const store = useAppStore()

const activeSection = ref('recents')

const sections = [
  { id: 'recents', icon: Clock, label: 'panel.recents' },
  { id: 'models', icon: Package, label: 'panel.models' },
  { id: 'transcription', icon: AudioLines, label: 'panel.transcription' },
  { id: 'processing', icon: Sparkles, label: 'panel.processing' },
  { id: 'dictionary', icon: BookOpen, label: 'panel.dictionary' },
  { id: 'shortcuts', icon: Keyboard, label: 'panel.shortcuts' },
  { id: 'microphone', icon: Mic, label: 'panel.microphone' },
  { id: 'providers', icon: Cloud, label: 'panel.providers' },
  { id: 'permissions', icon: Shield, label: 'panel.permissions' },
  { id: 'general', icon: Settings2, label: 'panel.general' },
]

const statusLabel = () => {
  if (store.isRecording) return t('status.recording')
  if (store.isTranscribing) return t('status.transcribing')
  return t('status.idle')
}

const statusClass = () => {
  if (store.isRecording) return 'recording'
  if (store.isTranscribing) return 'transcribing'
  return 'idle'
}

onMounted(async () => {
  getCurrentWindow().setTitle(t('window.panel'))
})
</script>

<template>
  <div class="flex h-full min-w-0 select-none">
    <!-- Sidebar -->
    <div class="backdrop-blur-[20px] backdrop-saturate-[1.8] bg-[hsl(var(--background)/0.72)] dark:bg-[hsl(var(--background)/0.65)] border-r-[0.5px] border-[hsl(var(--border)/0.5)] w-44 min-w-[10rem] flex flex-col flex-shrink-0">
      <!-- Drag region -->
      <div class="h-9 shrink-0 flex items-center px-2" data-tauri-drag-region />

      <!-- Nav items -->
      <nav class="flex-1 overflow-y-auto px-2 space-y-px">
        <button
          v-for="section in sections"
          :key="section.id"
          @click="activeSection = section.id"
          class="rounded-lg px-2.5 py-1.5 text-sm transition-all border border-transparent hover:bg-sidebar-hover-bg w-full text-left"
          :class="activeSection === section.id ? 'bg-sidebar-active-bg border-sidebar-active-border font-medium' : ''"
        >
          <div class="flex items-center gap-2">
            <component :is="section.icon" class="w-[18px] h-[18px] flex-shrink-0" :class="activeSection === section.id ? 'opacity-100 text-panel-accent' : 'opacity-70'" />
            <span class="text-[13px] truncate">{{ t(section.label) }}</span>
          </div>
        </button>
      </nav>

      <!-- Status indicator -->
      <div class="px-2.5 py-2 border-t border-panel-divider">
        <div class="flex items-center gap-1.5">
          <span
            class="inline-block w-2 h-2 rounded-full"
            :class="{
              'bg-emerald-500': statusClass() === 'idle',
              'bg-red-500 animate-status-pulse': statusClass() === 'recording',
              'bg-amber-500 animate-status-pulse': statusClass() === 'transcribing',
            }"
          />
          <span class="text-[11px] text-muted-foreground">{{ statusLabel() }}</span>
        </div>
      </div>
    </div>

    <!-- Content -->
    <div class="bg-[linear-gradient(160deg,var(--panel-bg-start),var(--panel-bg-end))] flex-1 min-w-0 flex flex-col overflow-hidden">
      <!-- Drag region for content area -->
      <div class="h-9 shrink-0" data-tauri-drag-region />

      <div class="flex-1 overflow-y-auto px-5 pb-5 [&::-webkit-scrollbar]:w-1.5 [&::-webkit-scrollbar-thumb]:bg-panel-scrollbar [&::-webkit-scrollbar-thumb]:rounded-[3px] [&::-webkit-scrollbar-track]:bg-transparent">
        <Transition name="fade" mode="out-in">
          <RecentsSection v-if="activeSection === 'recents'" key="recents" />
          <ModelsSection v-else-if="activeSection === 'models'" key="models" />
          <TranscriptionSection v-else-if="activeSection === 'transcription'" key="transcription" />
          <ProcessingSection v-else-if="activeSection === 'processing'" key="processing" @navigate="activeSection = $event" />
          <DictionarySection v-else-if="activeSection === 'dictionary'" key="dictionary" />
          <ShortcutsSection v-else-if="activeSection === 'shortcuts'" key="shortcuts" />
          <MicrophoneSection v-else-if="activeSection === 'microphone'" key="microphone" />
          <ProvidersSection v-else-if="activeSection === 'providers'" key="providers" />
          <PermissionsSection v-else-if="activeSection === 'permissions'" key="permissions" />
          <GeneralSection v-else-if="activeSection === 'general'" key="general" />
        </Transition>
      </div>
    </div>
  </div>
</template>
