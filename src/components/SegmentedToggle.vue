<script setup lang="ts">
defineProps<{
  modelValue: string
  options: { value: string; label: string }[]
  block?: boolean
}>()

const emit = defineEmits<{
  'update:modelValue': [value: string]
}>()
</script>

<template>
  <div
    class="rounded-md border border-border overflow-hidden"
    :class="block ? 'inline-flex w-full' : 'inline-flex'"
  >
    <button
      v-for="(option, index) in options"
      :key="option.value"
      class="px-3 py-1.5 text-sm transition-colors"
      :class="[
        modelValue === option.value
          ? 'bg-accent text-accent-foreground'
          : 'hover:bg-accent/50 text-muted-foreground',
        block ? 'flex-1' : 'whitespace-nowrap',
        index > 0 ? 'border-l border-border' : '',
      ]"
      @click="emit('update:modelValue', option.value)"
    >
      {{ option.label }}
    </button>
  </div>
</template>
