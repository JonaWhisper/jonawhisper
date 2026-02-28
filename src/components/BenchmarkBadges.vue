<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

const { t } = useI18n()

const props = defineProps<{
  wer: number | null
  rtf: number | null
  compact?: boolean
}>()

const werInfo = computed(() => {
  if (props.wer == null) return null
  if (props.wer < 3) return { label: t('benchmark.wer.excellent'), color: 'text-emerald-500' }
  if (props.wer < 5) return { label: t('benchmark.wer.good'), color: 'text-blue-500' }
  if (props.wer < 8) return { label: t('benchmark.wer.fair'), color: 'text-amber-500' }
  return { label: t('benchmark.wer.basic'), color: 'text-orange-500' }
})

const rtfInfo = computed(() => {
  if (props.rtf == null) return null
  if (props.rtf < 0.05) return { label: t('benchmark.rtf.lightning'), color: 'text-violet-500' }
  if (props.rtf < 0.15) return { label: t('benchmark.rtf.fast'), color: 'text-emerald-500' }
  if (props.rtf < 0.35) return { label: t('benchmark.rtf.normal'), color: 'text-blue-500' }
  return { label: t('benchmark.rtf.slow'), color: 'text-amber-500' }
})
</script>

<template>
  <span v-if="werInfo || rtfInfo" class="inline-flex items-center gap-1.5">
    <span v-if="werInfo" class="inline-flex items-center gap-0.5">
      <span :class="[werInfo.color, compact ? 'text-[10px]' : 'text-[11px]']" class="font-medium">{{ werInfo.label }}</span>
      <span :class="compact ? 'text-[9px]' : 'text-[10px]'" class="text-muted-foreground">{{ wer }}%</span>
    </span>
    <span v-if="rtfInfo" class="inline-flex items-center gap-0.5">
      <span :class="[rtfInfo.color, compact ? 'text-[10px]' : 'text-[11px]']" class="font-medium">{{ rtfInfo.label }}</span>
      <span :class="compact ? 'text-[9px]' : 'text-[10px]'" class="text-muted-foreground">{{ rtf }}x</span>
    </span>
  </span>
</template>
