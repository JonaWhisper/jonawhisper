<script setup lang="ts">
import { computed } from 'vue'

const props = withDefaults(defineProps<{
  spectrum: number[]
  barColor?: string
  size?: 'sm' | 'md'
}>(), {
  size: 'sm',
})

const config = computed(() =>
  props.size === 'md'
    ? { height: 48, barMax: 44, barWidth: 'w-1.5', gap: 'gap-1' }
    : { height: 32, barMax: 28, barWidth: 'w-1', gap: 'gap-[3px]' }
)

const bars = computed(() =>
  props.spectrum.map(level => Math.max(2, level * config.value.barMax))
)
</script>

<template>
  <div
    class="flex items-center justify-center"
    :class="[config.gap]"
    :style="{ height: `${config.height}px` }"
  >
    <div
      v-for="(height, i) in bars"
      :key="i"
      class="rounded-full transition-[height] duration-75"
      :class="[config.barWidth, barColor ?? 'bg-foreground']"
      :style="{ height: `${height}px` }"
    />
  </div>
</template>
