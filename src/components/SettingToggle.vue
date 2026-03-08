<script setup lang="ts">
import { Switch } from '@/components/ui/switch'

withDefaults(defineProps<{
  settingKey: string
  modelValue: boolean
  label: string
  description?: string
  disabled?: boolean
  borderTop?: boolean
}>(), {
  borderTop: true,
})

const emit = defineEmits<{
  'update:modelValue': [value: boolean, key: string]
}>()
</script>

<template>
  <div
    class="flex items-center justify-between py-2 gap-3"
    :class="[
      borderTop ? 'border-t-[0.5px] border-panel-divider' : '',
      disabled ? 'opacity-40' : '',
    ]"
  >
    <div>
      <div class="text-[13px] text-foreground">{{ label }}</div>
      <div v-if="description" class="text-[11px] text-muted-foreground mt-px">{{ description }}</div>
    </div>
    <Switch
      :model-value="modelValue"
      :disabled="disabled"
      @update:model-value="(v: boolean) => emit('update:modelValue', v, settingKey)"
    />
  </div>
</template>
