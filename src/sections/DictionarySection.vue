<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import { Plus, Trash2, ArrowRightLeft, ArrowRight, Type, Sparkles, Loader2 } from 'lucide-vue-next'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { useSettingsStore } from '@/stores/settings'

interface UserDictEntry {
  value: string
  kind: 'word' | 'mapping'
}

interface DictSuggestion {
  word: string
  count: number
}

/** Below this, a word is more likely a one-off ASR slip than the user's vocabulary. */
const MIN_OCCURRENCES = 3

const { t } = useI18n()
const settings = useSettingsStore()

const entries = ref<UserDictEntry[]>([])
const newWord = ref('')
const newPattern = ref('')
const newReplacement = ref('')
const newKind = ref<'word' | 'mapping'>('word')

async function load() {
  entries.value = await invoke<UserDictEntry[]>('get_user_dict')
}

async function save() {
  await invoke('save_user_dict', { entries: entries.value })
}

function addEntry() {
  let val: string
  let kind: 'word' | 'mapping'

  if (newKind.value === 'mapping') {
    const pattern = newPattern.value.trim()
    const replacement = newReplacement.value.trim()
    if (!pattern || !replacement) return
    val = `${pattern}=${replacement}`
    kind = 'mapping'
  } else {
    val = newWord.value.trim()
    if (!val) return
    kind = 'word'
  }

  // Avoid duplicates
  if (entries.value.some(e => e.value === val)) return
  entries.value.push({ value: val, kind })
  newWord.value = ''
  newPattern.value = ''
  newReplacement.value = ''
  save()
}

function removeEntry(index: number) {
  entries.value.splice(index, 1)
  save()
}

/** Split a mapping value "pattern=replacement" for display */
function mappingParts(value: string): [string, string] {
  const idx = value.indexOf('=')
  if (idx === -1) return [value, '']
  return [value.slice(0, idx), value.slice(idx + 1)]
}

const suggestions = ref<DictSuggestion[]>([])
const suggesting = ref(false)
const suggested = ref(false)
const suggestError = ref('')

async function loadSuggestions() {
  suggesting.value = true
  suggestError.value = ''
  try {
    suggestions.value = await invoke<DictSuggestion[]>('suggest_user_dict_words', {
      language: settings.selectedLanguage,
      minCount: MIN_OCCURRENCES,
    })
    suggested.value = true
  } catch (e) {
    suggestError.value = String(e)
  } finally {
    suggesting.value = false
  }
}

function acceptSuggestion(s: DictSuggestion) {
  if (!entries.value.some(e => e.value === s.word)) {
    entries.value.push({ value: s.word, kind: 'word' })
    save()
  }
  suggestions.value = suggestions.value.filter(x => x.word !== s.word)
}

function dismissSuggestion(s: DictSuggestion) {
  suggestions.value = suggestions.value.filter(x => x.word !== s.word)
}

onMounted(load)
</script>

<template>
  <div>
    <div class="text-[20px] font-bold tracking-[-0.02em] mb-1">{{ t('panel.dictionary') }}</div>
    <div class="text-[12px] text-muted-foreground mb-4">{{ t('dictionary.description') }}</div>

    <!-- Add entry card -->
    <div class="bg-panel-card-bg backdrop-blur-sm border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('dictionary.add') }}</div>

      <!-- Word input -->
      <div v-if="newKind === 'word'" class="flex items-center gap-2">
        <input
          v-model="newWord"
          :placeholder="t('dictionary.placeholder.word')"
          class="flex-1 h-8 rounded-md border border-input bg-background px-3 text-xs placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          @keydown.enter="addEntry"
        />
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger as-child>
              <button
                class="inline-flex items-center justify-center rounded-md border h-8 w-8 hover:bg-accent hover:text-accent-foreground shrink-0 transition-colors text-blue-500 border-blue-500/30 bg-background"
                :aria-label="t('dictionary.toggleKind')"
                @click="newKind = 'mapping'"
              >
                <Type class="h-3.5 w-3.5" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="top">{{ t('dictionary.kind.word') }}</TooltipContent>
          </Tooltip>
        </TooltipProvider>
        <button
          class="inline-flex items-center justify-center rounded-md bg-primary text-primary-foreground h-8 w-8 hover:bg-primary/90 shrink-0 transition-colors disabled:opacity-40"
          :disabled="!newWord.trim()"
          :aria-label="t('dictionary.add')"
          @click="addEntry"
        >
          <Plus class="h-4 w-4" />
        </button>
      </div>

      <!-- Mapping inputs (pattern → replacement) -->
      <div v-else class="flex items-center gap-2">
        <input
          v-model="newPattern"
          :placeholder="t('dictionary.placeholder.pattern')"
          class="flex-1 h-8 rounded-md border border-input bg-background px-3 text-xs placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
        />
        <ArrowRight class="h-3.5 w-3.5 text-muted-foreground shrink-0" />
        <input
          v-model="newReplacement"
          :placeholder="t('dictionary.placeholder.replacement')"
          class="flex-1 h-8 rounded-md border border-input bg-background px-3 text-xs placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
          @keydown.enter="addEntry"
        />
        <TooltipProvider>
          <Tooltip>
            <TooltipTrigger as-child>
              <button
                class="inline-flex items-center justify-center rounded-md border h-8 w-8 hover:bg-accent hover:text-accent-foreground shrink-0 transition-colors text-amber-500 border-amber-500/30 bg-background"
                :aria-label="t('dictionary.toggleKind')"
                @click="newKind = 'word'"
              >
                <ArrowRightLeft class="h-3.5 w-3.5" />
              </button>
            </TooltipTrigger>
            <TooltipContent side="top">{{ t('dictionary.kind.mapping') }}</TooltipContent>
          </Tooltip>
        </TooltipProvider>
        <button
          class="inline-flex items-center justify-center rounded-md bg-primary text-primary-foreground h-8 w-8 hover:bg-primary/90 shrink-0 transition-colors disabled:opacity-40"
          :disabled="!newPattern.trim() || !newReplacement.trim()"
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

    <!-- Suggestions from history card -->
    <div class="bg-panel-card-bg backdrop-blur-sm border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="flex items-center justify-between mb-2.5">
        <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground">
          {{ t('dictionary.suggestions') }}
          <span v-if="suggestions.length" class="ml-1 opacity-60">({{ suggestions.length }})</span>
        </div>
        <button
          class="inline-flex items-center gap-1.5 rounded-md border border-input bg-background h-7 px-2.5 text-[11px] hover:bg-accent hover:text-accent-foreground transition-colors disabled:opacity-40"
          :disabled="suggesting"
          @click="loadSuggestions"
        >
          <Loader2 v-if="suggesting" class="h-3 w-3 animate-spin" />
          <Sparkles v-else class="h-3 w-3" />
          {{ t('dictionary.suggestions.analyze') }}
        </button>
      </div>

      <div class="text-[12px] text-muted-foreground mb-2">{{ t('dictionary.suggestions.hint') }}</div>

      <div v-if="suggestError" class="text-[12px] text-destructive py-2">{{ suggestError }}</div>
      <div v-else-if="suggested && !suggestions.length" class="text-[13px] text-muted-foreground py-3 text-center">
        {{ t('dictionary.suggestions.empty') }}
      </div>

      <div
        v-else-if="suggestions.length"
        class="divide-y divide-panel-divider max-h-[260px] overflow-y-auto pr-1.5 [&::-webkit-scrollbar]:w-1.5 [&::-webkit-scrollbar-thumb]:bg-panel-scrollbar [&::-webkit-scrollbar-thumb]:rounded-[3px] [&::-webkit-scrollbar-track]:bg-transparent"
      >
        <div v-for="s in suggestions" :key="s.word" class="flex items-center justify-between py-2 gap-3">
          <div class="flex items-center gap-2 min-w-0">
            <span class="text-[13px] truncate">{{ s.word }}</span>
            <span class="text-[11px] text-muted-foreground shrink-0">{{ t('dictionary.suggestions.count', { n: s.count }) }}</span>
          </div>
          <div class="flex items-center gap-1 shrink-0">
            <button
              class="inline-flex items-center justify-center rounded-md h-7 w-7 hover:bg-accent transition-colors text-muted-foreground"
              :aria-label="t('dictionary.suggestions.dismiss')"
              @click="dismissSuggestion(s)"
            >
              <Trash2 class="h-3.5 w-3.5" />
            </button>
            <button
              class="inline-flex items-center justify-center rounded-md bg-primary text-primary-foreground h-7 w-7 hover:bg-primary/90 transition-colors"
              :aria-label="t('dictionary.add')"
              @click="acceptSuggestion(s)"
            >
              <Plus class="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
      </div>
    </div>

    <!-- Entries list card -->
    <div class="bg-panel-card-bg backdrop-blur-sm border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px]">
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
            <TooltipProvider>
              <Tooltip>
                <TooltipTrigger as-child>
                  <span
                    class="inline-flex items-center justify-center w-5 h-5 rounded shrink-0"
                    :class="entry.kind === 'mapping' ? 'text-amber-500 bg-amber-500/10' : 'text-blue-500 bg-blue-500/10'"
                  >
                    <ArrowRightLeft v-if="entry.kind === 'mapping'" class="h-3 w-3" />
                    <Type v-else class="h-3 w-3" />
                  </span>
                </TooltipTrigger>
                <TooltipContent side="right">
                  <span v-if="entry.kind === 'word'">{{ t('dictionary.kind.word') }}</span>
                  <span v-else>{{ t('dictionary.kind.mapping') }}</span>
                </TooltipContent>
              </Tooltip>
            </TooltipProvider>
            <!-- Word: simple text. Mapping: pattern → replacement -->
            <template v-if="entry.kind === 'mapping'">
              <span class="text-[13px] text-foreground truncate">{{ mappingParts(entry.value)[0] }}</span>
              <ArrowRight class="h-3 w-3 text-muted-foreground shrink-0" />
              <span class="text-[13px] text-foreground truncate">{{ mappingParts(entry.value)[1] }}</span>
            </template>
            <span v-else class="text-[13px] text-foreground truncate">{{ entry.value }}</span>
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
