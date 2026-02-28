<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Badge } from '@/components/ui/badge'

const { t } = useI18n()

const props = defineProps<{
  wer: number | null
  rtf: number | null
  compact?: boolean
}>()

const werInfo = computed(() => {
  if (props.wer == null) return null
  if (props.wer < 3) return { label: t('benchmark.wer.excellent'), bg: 'bg-emerald-500/10 text-emerald-600' }
  if (props.wer < 5) return { label: t('benchmark.wer.good'), bg: 'bg-blue-500/10 text-blue-600' }
  if (props.wer < 8) return { label: t('benchmark.wer.fair'), bg: 'bg-amber-500/10 text-amber-600' }
  return { label: t('benchmark.wer.basic'), bg: 'bg-orange-500/10 text-orange-600' }
})

const rtfInfo = computed(() => {
  if (props.rtf == null) return null
  if (props.rtf < 0.05) return { label: t('benchmark.rtf.lightning'), bg: 'bg-violet-500/10 text-violet-600' }
  if (props.rtf < 0.15) return { label: t('benchmark.rtf.fast'), bg: 'bg-emerald-500/10 text-emerald-600' }
  if (props.rtf < 0.35) return { label: t('benchmark.rtf.normal'), bg: 'bg-blue-500/10 text-blue-600' }
  return { label: t('benchmark.rtf.slow'), bg: 'bg-amber-500/10 text-amber-600' }
})
</script>

<template>
  <span v-if="werInfo || rtfInfo" class="inline-flex items-center gap-1">
    <Badge
      v-if="werInfo"
      variant="secondary"
      :class="[werInfo.bg, 'border-transparent font-medium', compact ? 'text-[9px] px-1 py-0' : 'text-[10px] px-1.5 py-0']"
    >
      {{ werInfo.label }} {{ +wer!.toFixed(1) }}%
    </Badge>
    <Badge
      v-if="rtfInfo"
      variant="secondary"
      :class="[rtfInfo.bg, 'border-transparent font-medium', compact ? 'text-[9px] px-1 py-0' : 'text-[10px] px-1.5 py-0']"
    >
      {{ rtfInfo.label }} {{ +rtf!.toFixed(2) }}x
    </Badge>
  </span>
</template>
