<script setup lang="ts">
import { Type, SpellCheck, MessageSquare, Cloud, Cpu, Scissors, ShieldCheck } from 'lucide-vue-next'

defineProps<{
  type: 'local' | 'cloud' | 'bert' | 'punctuation' | 'correction' | 'llm' | 'vad' | 'hallucination'
}>()

const config = {
  local:         { bg: 'bg-blue-500/10 text-blue-600 dark:text-blue-400', icon: Cpu },
  cloud:         { bg: 'bg-sky-500/10 text-sky-600 dark:text-sky-400', icon: Cloud },
  bert:          { bg: 'bg-violet-500/10 text-violet-600 dark:text-violet-400', icon: Type },
  punctuation:   { bg: 'bg-violet-500/10 text-violet-600 dark:text-violet-400', icon: Type },
  correction:    { bg: 'bg-amber-500/10 text-amber-600 dark:text-amber-400', icon: SpellCheck },
  llm:           { bg: 'bg-blue-500/10 text-blue-600 dark:text-blue-400', icon: MessageSquare },
  vad:           { bg: 'bg-emerald-500/10 text-emerald-600 dark:text-emerald-400', icon: Scissors },
  hallucination: { bg: 'bg-rose-500/10 text-rose-600 dark:text-rose-400', icon: ShieldCheck },
}
</script>

<template>
  <span
    class="inline-flex items-center gap-0.5 rounded px-1.5 py-0.5 text-[10px] font-medium shrink-0"
    :class="config[type].bg"
  >
    <component :is="config[type].icon" class="h-2.5 w-2.5" />
    <slot />
  </span>
</template>
