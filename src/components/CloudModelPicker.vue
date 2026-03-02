<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Input } from '@/components/ui/input'
import { Button } from '@/components/ui/button'
import {
  Select, SelectContent, SelectItem, SelectTrigger, SelectValue,
} from '@/components/ui/select'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { RefreshCw, Loader2 } from 'lucide-vue-next'

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

const isCustom = computed(() => {
  if (props.modelOptions.length === 0) return true
  return !props.modelOptions.includes(props.modelValue)
})

const selectValue = computed(() => {
  if (props.modelOptions.length === 0) return CUSTOM_VALUE
  if (props.modelOptions.includes(props.modelValue)) return props.modelValue
  return CUSTOM_VALUE
})

function onSelect(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value !== 'string') return
  if (value === CUSTOM_VALUE) {
    emit('update:modelValue', '')
    return
  }
  emit('update:modelValue', value)
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
    <Select v-if="modelOptions.length > 0" :model-value="selectValue" @update:model-value="onSelect">
      <SelectTrigger class="w-auto min-w-[140px] h-8 text-xs">
        <SelectValue />
      </SelectTrigger>
      <SelectContent>
        <SelectItem v-for="m in modelOptions" :key="m" :value="m">{{ m }}</SelectItem>
        <SelectItem :value="CUSTOM_VALUE">{{ t('settings.cloudAsr.custom') }}</SelectItem>
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
  <div v-if="modelOptions.length > 0 && isCustom" class="wf-form-row">
    <div>
      <div class="wf-form-label">{{ t('settings.cloudAsr.customPlaceholder') }}</div>
    </div>
    <Input
      :value="modelValue"
      @input="onInput"
      :placeholder="t('settings.cloudAsr.customPlaceholder')"
      class="h-8 text-xs min-w-[140px] max-w-[200px]"
    />
  </div>
</template>
