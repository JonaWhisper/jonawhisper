<script setup lang="ts">
import { computed, ref, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { RefreshCw, Loader2, Search } from 'lucide-vue-next'

const props = defineProps<{
  modelOptions: string[]
  modelValue: string
  refreshing: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
  'refresh': []
}>()

const { t } = useI18n()

const CUSTOM_VALUE = '_custom'
const SEARCH_THRESHOLD = 10

const search = ref('')
const searchInput = ref<HTMLInputElement | null>()

const isCustom = computed(() => {
  if (props.modelOptions.length === 0) return true
  return !props.modelOptions.includes(props.modelValue)
})

const selectValue = computed(() => {
  if (props.modelOptions.length === 0) return CUSTOM_VALUE
  if (props.modelOptions.includes(props.modelValue)) return props.modelValue
  return CUSTOM_VALUE
})

const filteredOptions = computed(() => {
  if (!search.value) return props.modelOptions
  const q = search.value.toLowerCase()
  return props.modelOptions.filter(m => m.toLowerCase().includes(q))
})

const showSearch = computed(() => props.modelOptions.length >= SEARCH_THRESHOLD)

function onSelect(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  if (value === CUSTOM_VALUE) {
    emit('update:modelValue', '')
    return
  }
  emit('update:modelValue', value)
}

function onOpenChange(open: boolean) {
  if (open) {
    search.value = ''
    nextTick(() => searchInput.value?.focus())
  }
}

// Prevent select from closing when typing in search
function onSearchKeydown(e: KeyboardEvent) {
  e.stopPropagation()
}

let debounce: ReturnType<typeof setTimeout> | null = null
const localInput = ref(props.modelValue)

function onInput(event: Event) {
  const value = (event.target as HTMLInputElement).value
  localInput.value = value
  if (debounce) clearTimeout(debounce)
  debounce = setTimeout(() => {
    emit('update:modelValue', value)
  }, 500)
}
</script>

<template>
  <div class="flex items-center gap-2">
    <Select v-if="modelOptions.length > 0" :model-value="selectValue" @update:model-value="onSelect" @update:open="onOpenChange">
      <SelectTrigger class="w-auto min-w-[140px] max-w-[260px] h-8 text-xs">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <template v-if="showSearch" #header>
          <div class="flex items-center gap-1.5 px-2 py-1.5 border-b border-border/50">
            <Search class="w-3.5 h-3.5 text-muted-foreground shrink-0" />
            <input
              ref="searchInput"
              v-model="search"
              type="text"
              class="flex-1 bg-transparent text-xs outline-none placeholder:text-muted-foreground/60"
              :placeholder="t('provider.searchPreset')"
              @keydown="onSearchKeydown"
            />
          </div>
        </template>
        <div v-if="showSearch && filteredOptions.length === 0" class="py-3 text-center text-xs text-muted-foreground">
          {{ t('provider.noResults') }}
        </div>
        <SelectItem v-for="m in filteredOptions" :key="m" :value="m">{{ m }}</SelectItem>
        <SelectItem :value="CUSTOM_VALUE" class="border-t border-border/50 mt-1">{{ t('settings.cloudAsr.custom') }}</SelectItem>
      </SelectContent>
    </Select>
    <Input
      v-else
      :value="modelValue"
      @input="onInput"
      :placeholder="t('settings.cloudAsr.customPlaceholder')"
      class="h-8 text-xs min-w-[140px]"
    />
    <TooltipProvider :delay-duration="300">
      <Tooltip>
        <TooltipTrigger as-child>
          <Button variant="outline" size="icon" class="h-8 w-8 shrink-0" :disabled="refreshing" @click="$emit('refresh')">
            <Loader2 v-if="refreshing" class="w-3.5 h-3.5 animate-spin" />
            <RefreshCw v-else class="w-3.5 h-3.5" />
          </Button>
        </TooltipTrigger>
        <TooltipContent side="bottom" :side-offset="4">{{ t('settings.models.refresh') }}</TooltipContent>
      </Tooltip>
    </TooltipProvider>
  </div>
  <!-- Custom model input (shown below when "Custom" selected from dropdown) -->
  <div v-if="modelOptions.length > 0 && isCustom" class="flex items-center justify-between py-2 gap-3 border-t-[0.5px] border-panel-divider">
    <div>
      <div class="text-[13px] text-foreground">{{ t('settings.cloudAsr.customPlaceholder') }}</div>
    </div>
    <Input
      :value="modelValue"
      @input="onInput"
      :placeholder="t('settings.cloudAsr.customPlaceholder')"
      class="h-8 text-xs min-w-[140px] max-w-[200px]"
    />
  </div>
</template>
