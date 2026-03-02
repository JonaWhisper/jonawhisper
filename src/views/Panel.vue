<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { useAppStore } from '@/stores/app'
import { Clock, Package, AudioLines, Sparkles, Keyboard, Mic, Cloud, Settings2 } from 'lucide-vue-next'
import RecentsSection from '@/sections/RecentsSection.vue'
import ModelsSection from '@/sections/ModelsSection.vue'
import TranscriptionSection from '@/sections/TranscriptionSection.vue'
import ProcessingSection from '@/sections/ProcessingSection.vue'
import ShortcutsSection from '@/sections/ShortcutsSection.vue'
import MicrophoneSection from '@/sections/MicrophoneSection.vue'
import ProvidersSection from '@/sections/ProvidersSection.vue'
import GeneralSection from '@/sections/GeneralSection.vue'

const { t } = useI18n()
const store = useAppStore()

const activeSection = ref('recents')

const sections = [
  { id: 'recents', icon: Clock, label: 'panel.recents' },
  { id: 'models', icon: Package, label: 'panel.models' },
  { id: 'transcription', icon: AudioLines, label: 'panel.transcription' },
  { id: 'processing', icon: Sparkles, label: 'panel.processing' },
  { id: 'shortcuts', icon: Keyboard, label: 'panel.shortcuts' },
  { id: 'microphone', icon: Mic, label: 'panel.microphone' },
  { id: 'providers', icon: Cloud, label: 'panel.providers' },
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
    <div class="panel-sidebar w-44 min-w-[10rem] flex flex-col flex-shrink-0">
      <!-- Drag region -->
      <div class="h-8 shrink-0" data-tauri-drag-region />

      <!-- Nav items -->
      <nav class="flex-1 overflow-y-auto px-2 space-y-0.5">
        <button
          v-for="section in sections"
          :key="section.id"
          @click="activeSection = section.id"
          class="nav-pill w-full text-left"
          :class="{ active: activeSection === section.id }"
        >
          <div class="flex items-center gap-2">
            <component :is="section.icon" class="w-4 h-4 flex-shrink-0 opacity-60" />
            <span class="text-sm font-medium truncate">{{ t(section.label) }}</span>
          </div>
        </button>
      </nav>

      <!-- Status indicator -->
      <div class="px-3 py-2.5 border-t border-border/50">
        <div class="flex items-center gap-2">
          <span class="status-dot" :class="statusClass()" />
          <span class="text-xs text-muted-foreground">{{ statusLabel() }}</span>
        </div>
      </div>
    </div>

    <!-- Content -->
    <div class="flex-1 min-w-0 overflow-y-auto">
      <!-- Drag region for content area too -->
      <div class="h-8 shrink-0" data-tauri-drag-region />

      <div class="px-5 pb-5">
        <!-- Section title -->
        <h2 class="text-lg font-semibold mb-4">
          {{ t(sections.find(s => s.id === activeSection)?.label ?? '') }}
        </h2>

        <Transition name="fade" mode="out-in">
          <RecentsSection v-if="activeSection === 'recents'" key="recents" />
          <ModelsSection v-else-if="activeSection === 'models'" key="models" />
          <TranscriptionSection v-else-if="activeSection === 'transcription'" key="transcription" />
          <ProcessingSection v-else-if="activeSection === 'processing'" key="processing" />
          <ShortcutsSection v-else-if="activeSection === 'shortcuts'" key="shortcuts" />
          <MicrophoneSection v-else-if="activeSection === 'microphone'" key="microphone" />
          <ProvidersSection v-else-if="activeSection === 'providers'" key="providers" />
          <GeneralSection v-else-if="activeSection === 'general'" key="general" />
        </Transition>
      </div>
    </div>
  </div>
</template>
