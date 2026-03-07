<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Badge } from '@/components/ui/badge'
import TypeBadge from '@/components/TypeBadge.vue'

const { t } = useI18n()

defineProps<{
  label: string
  type?: 'bert' | 'punctuation' | 'correction' | 'llm'
  location: 'local' | 'cloud'
  recommended?: boolean
  compact?: boolean
}>()
</script>

<template>
  <span :class="compact ? 'inline-flex items-center gap-1.5 truncate' : 'flex items-center gap-1.5'">
    <span :class="{ 'truncate': compact }">{{ label }}</span>
    <Badge v-if="!compact && recommended" variant="secondary" class="text-[9px] px-1 py-0 bg-emerald-500/10 text-emerald-600 border-transparent font-medium">{{ t('settings.cleanup.recommended') }}</Badge>
    <TypeBadge v-if="type" :type="type" :class="{ 'ml-auto': !compact }" />
    <TypeBadge :type="location" :class="{ 'ml-auto': !compact && !type }" />
  </span>
</template>
