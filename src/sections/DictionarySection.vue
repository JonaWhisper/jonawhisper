<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { Plus, Trash2, ArrowRightLeft, Type } from 'lucide-vue-next'

interface UserDictEntry {
  value: string
  kind: 'word' | 'mapping'
}

const { t } = useI18n()

const entries = ref<UserDictEntry[]>([])
const newValue = ref('')
const newKind = ref<'word' | 'mapping'>('word')
const dirty = ref(false)

async function load() {
  entries.value = await invoke<UserDictEntry[]>('get_user_dict')
  dirty.value = false
}

async function save() {
  await invoke('save_user_dict', { entries: entries.value })
  dirty.value = false
}

function addEntry() {
  const val = newValue.value.trim()
  if (!val) return
  // Detect kind from content: contains "=" → mapping
  const kind = val.includes('=') ? 'mapping' : newKind.value
  // Avoid duplicates
  if (entries.value.some(e => e.value === val)) return
  entries.value.push({ value: val, kind })
  newValue.value = ''
  dirty.value = true
  save()
}

function removeEntry(index: number) {
  entries.value.splice(index, 1)
  dirty.value = true
  save()
}

onMounted(load)
</script>

<template>
  <div>
    <div class="text-[20px] font-bold tracking-[-0.02em] mb-1">{{ t('panel.dictionary') }}</div>
    <div class="text-[12px] text-muted-foreground mb-4">{{ t('dictionary.description') }}</div>

    <!-- Add entry card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('dictionary.add') }}</div>

      <div class="flex items-center gap-2">
        <input
          v-model="newValue"
          :placeholder="newKind === 'word' ? t('dictionary.placeholder.word') : t('dictionary.placeholder.mapping')"
          class="flex-1 h-8 rounded-md border border-input bg-background px-3 text-xs placeholder:text-muted-foreground focus:outline-none focus:ring-1 focus:ring-ring"
          @keydown.enter="addEntry"
        />
        <button
          class="inline-flex items-center justify-center rounded-md border border-input bg-background h-8 w-8 hover:bg-accent hover:text-accent-foreground shrink-0 transition-colors"
          :class="newKind === 'mapping' ? 'text-amber-500 border-amber-500/30' : 'text-blue-500 border-blue-500/30'"
          :aria-label="t('dictionary.toggleKind')"
          @click="newKind = newKind === 'word' ? 'mapping' : 'word'"
        >
          <ArrowRightLeft v-if="newKind === 'mapping'" class="h-3.5 w-3.5" />
          <Type v-else class="h-3.5 w-3.5" />
        </button>
        <button
          class="inline-flex items-center justify-center rounded-md bg-primary text-primary-foreground h-8 w-8 hover:bg-primary/90 shrink-0 transition-colors disabled:opacity-40"
          :disabled="!newValue.trim()"
          :aria-label="t('dictionary.add')"
          @click="addEntry"
        >
          <Plus class="h-4 w-4" />
        </button>
      </div>

      <div class="text-[11px] text-muted-foreground mt-2">
        <span v-if="newKind === 'word'">{{ t('dictionary.hint.word') }}</span>
        <span v-else>{{ t('dictionary.hint.mapping') }}</span>
      </div>
    </div>

    <!-- Entries list card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px]">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">
        {{ t('dictionary.entries') }}
        <span v-if="entries.length" class="ml-1 opacity-60">({{ entries.length }})</span>
      </div>

      <div v-if="!entries.length" class="text-[13px] text-muted-foreground py-4 text-center">
        {{ t('dictionary.empty') }}
      </div>

      <div v-else class="divide-y divide-panel-divider">
        <div
          v-for="(entry, i) in entries"
          :key="i"
          class="flex items-center justify-between py-2 gap-3 group"
        >
          <div class="flex items-center gap-2 min-w-0">
            <span
              class="inline-flex items-center justify-center w-5 h-5 rounded shrink-0"
              :class="entry.kind === 'mapping' ? 'text-amber-500 bg-amber-500/10' : 'text-blue-500 bg-blue-500/10'"
            >
              <ArrowRightLeft v-if="entry.kind === 'mapping'" class="h-3 w-3" />
              <Type v-else class="h-3 w-3" />
            </span>
            <span class="text-[13px] text-foreground truncate">{{ entry.value }}</span>
          </div>
          <button
            class="inline-flex items-center justify-center rounded-md h-7 w-7 text-muted-foreground hover:text-destructive hover:bg-destructive/10 opacity-0 group-hover:opacity-100 transition-all shrink-0"
            :aria-label="t('aria.delete')"
            @click="removeEntry(i)"
          >
            <Trash2 class="h-3.5 w-3.5" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
