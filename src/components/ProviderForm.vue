<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import type { Provider, ProviderKind } from '@/stores/types'
import { PROVIDER_PRESETS, PRESET_ENTRIES } from '@/config/providers'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Loader2, CheckCircle2 } from 'lucide-vue-next'

const props = defineProps<{
  provider?: Provider
}>()

const emit = defineEmits<{
  save: [provider: Provider]
  cancel: []
}>()

const { t } = useI18n()

const isEditing = computed(() => !!props.provider)

const kind = ref<ProviderKind>(props.provider?.kind ?? 'Custom')
const name = ref(props.provider?.name ?? '')
const url = ref(props.provider?.url ?? '')
const apiKey = ref(props.provider?.api_key ?? '')
const errors = ref<Record<string, string>>({})

// Test state
const testStatus = ref<'idle' | 'loading' | 'success' | 'error'>('idle')
const testMessage = ref('')
const fetchedModels = ref<string[]>(props.provider?.cached_models ?? [])

const showUrl = computed(() => kind.value === 'Custom')
const canTest = computed(() => apiKey.value.trim().length > 0)

watch(kind, (newKind) => {
  if (isEditing.value) return
  const preset = PROVIDER_PRESETS[newKind]
  if (preset) {
    name.value = preset.label
    url.value = preset.url
  } else {
    name.value = ''
    url.value = ''
  }
  // Reset test state on kind change
  testStatus.value = 'idle'
  testMessage.value = ''
  fetchedModels.value = []
})

function onKindChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value === 'string') {
    kind.value = value as ProviderKind
  }
}

function validate(): boolean {
  errors.value = {}
  if (!name.value.trim()) errors.value.name = t('validation.required')
  if (showUrl.value && !url.value.trim()) errors.value.url = t('validation.required')
  return Object.keys(errors.value).length === 0
}

async function testConnection() {
  testStatus.value = 'loading'
  testMessage.value = ''

  const tempProvider: Provider = {
    id: props.provider?.id ?? 'temp',
    name: name.value.trim(),
    kind: kind.value,
    url: url.value.trim(),
    api_key: apiKey.value.trim(),
    cached_models: [],
  }

  try {
    const models = await invoke<string[]>('fetch_provider_models', { provider: tempProvider })
    fetchedModels.value = models
    testStatus.value = 'success'
    testMessage.value = t('provider.testSuccess', [models.length])
  } catch (e) {
    testStatus.value = 'error'
    const msg = String(e)
    // Truncate long error messages
    testMessage.value = msg.length > 120 ? msg.slice(0, 120) + '…' : msg
    fetchedModels.value = []
  }
}

function save() {
  if (!validate()) return

  const provider: Provider = {
    id: props.provider?.id ?? `provider-${kind.value.toLowerCase()}-${Date.now()}`,
    name: name.value.trim(),
    kind: kind.value,
    url: url.value.trim(),
    api_key: apiKey.value.trim(),
    cached_models: fetchedModels.value,
  }

  emit('save', provider)
}
</script>

<template>
  <div class="space-y-4">
    <!-- Add mode: kind selector -->
    <div v-if="!isEditing" class="space-y-2">
      <Label class="text-xs text-muted-foreground">{{ t('provider.kind') }}</Label>
      <Select :model-value="kind" @update:model-value="onKindChange">
        <SelectTrigger class="w-full h-9 text-sm">
          <SelectValue />
        </SelectTrigger>
        <SelectContent class="max-h-52">
          <SelectItem v-for="[kind, preset] in PRESET_ENTRIES" :key="kind" :value="kind">{{ preset.label }}</SelectItem>
          <SelectItem value="Custom">{{ t('provider.kind.custom') }}</SelectItem>
        </SelectContent>
      </Select>
    </div>

    <div class="space-y-2">
      <div class="flex items-center justify-between">
        <Label class="text-xs text-muted-foreground">{{ t('provider.name') }}</Label>
        <span v-if="isEditing" class="text-xs px-1.5 py-0.5 rounded bg-muted text-muted-foreground">{{ kind }}</span>
      </div>
      <Input v-model="name" class="h-9 text-sm" />
      <p v-if="errors.name" class="text-xs text-destructive">{{ errors.name }}</p>
    </div>

    <div v-if="showUrl" class="space-y-2">
      <Label class="text-xs text-muted-foreground">{{ t('provider.url') }}</Label>
      <Input v-model="url" class="h-9 text-sm" />
      <p v-if="errors.url" class="text-xs text-destructive">{{ errors.url }}</p>
    </div>

    <div class="space-y-2">
      <Label class="text-xs text-muted-foreground">{{ t('provider.apiKey') }}</Label>
      <div class="flex gap-2">
        <Input v-model="apiKey" type="password" placeholder="sk-..." class="h-9 text-sm flex-1" />
        <Button
          variant="outline"
          size="sm"
          class="shrink-0 h-9 w-20"
          :disabled="!canTest || testStatus === 'loading'"
          @click="testConnection"
        >
          <Loader2 v-if="testStatus === 'loading'" class="w-3.5 h-3.5 animate-spin" />
          <template v-else>{{ t('provider.test') }}</template>
        </Button>
      </div>
      <!-- Test result -->
      <div v-if="testStatus === 'success'" class="flex items-center gap-1.5 text-xs text-green-600">
        <CheckCircle2 class="w-3.5 h-3.5" />
        <span>{{ testMessage }}</span>
      </div>
      <p v-if="testStatus === 'error'" class="text-xs text-destructive">{{ testMessage }}</p>
    </div>

    <div class="flex justify-end gap-2 pt-2">
      <Button variant="outline" size="sm" @click="emit('cancel')">{{ t('modelManager.cancel') }}</Button>
      <Button size="sm" @click="save">{{ t('modelManager.save') }}</Button>
    </div>
  </div>
</template>
