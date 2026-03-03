<script setup lang="ts">
defineProps<{
  modelValue: string
  options: { value: string; label: string; badge?: string }[]
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
      <span
        v-if="option.badge"
        class="ml-1 rounded px-1 py-px text-[10px] font-medium text-green-600 dark:text-green-400"
        :class="modelValue === option.value ? 'bg-green-500/15' : 'bg-green-500/10'"
      >{{ option.badge }}</span>
    </button>
  </div>
</template>
