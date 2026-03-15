<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useSettingsStore } from '@/stores/settings'
import { Slider } from '@/components/ui/slider'

const { t } = useI18n()
const settings = useSettingsStore()

function onUpdate(v: number[] | undefined) {
  if (v?.[0] != null) settings.llmMaxTokens = v[0]
}

function onCommit(v: number[]) {
  const val = v[0] ?? settings.llmMaxTokens
  settings.setSetting('llm_max_tokens', String(val))
}
</script>

<template>
  <div class="flex items-center justify-between py-2 gap-3" :class="$attrs.class">
    <div>
      <div class="text-[13px] text-foreground">{{ t('settings.llm.maxTokens') }}</div>
    </div>
    <div class="flex items-center gap-2">
      <Slider
        class="w-24"
        :model-value="[settings.llmMaxTokens]"
        :min="128"
        :max="8192"
        :step="128"
        @update:model-value="onUpdate"
        @value-commit="onCommit"
      />
      <span class="text-xs text-muted-foreground tabular-nums min-w-8 text-right">{{ settings.llmMaxTokens }}</span>
    </div>
  </div>
</template>
