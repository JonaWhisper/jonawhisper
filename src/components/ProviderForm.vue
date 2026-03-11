<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import type { Provider } from '@/stores/types'
import { useEnginesStore } from '@/stores/engines'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
} from '@/components/ui/combobox'
import { ComboboxAnchor, ComboboxTrigger } from 'reka-ui'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Separator } from '@/components/ui/separator'
import { Switch } from '@/components/ui/switch'
import { Loader2, CheckCircle2, XCircle, ShieldAlert, ChevronsUpDown } from 'lucide-vue-next'

const props = defineProps<{
  provider?: Provider
}>()

const emit = defineEmits<{
  save: [provider: Provider]
  cancel: []
}>()

const { t, te } = useI18n()
const engines = useEnginesStore()

const isEditing = computed(() => !!props.provider)

const kind = ref(props.provider?.kind ?? (engines.providerPresets[0]?.id ?? 'custom'))
const name = ref(props.provider?.name ?? '')
const url = ref(props.provider?.url ?? '')
const apiKey = ref('')
const allowInsecure = ref(props.provider?.allow_insecure ?? false)
const supportsAsr = ref(props.provider?.supports_asr ?? true)
const supportsLlm = ref(props.provider?.supports_llm ?? true)
const errors = ref<Record<string, string>>({})
const extraValues = ref<Record<string, string>>({})

// Initialize extra values from existing provider or preset defaults
function initExtraValues(kindId: string) {
  const preset = engines.providerPresets.find(p => p.id === kindId)
  if (preset) {
    const vals: Record<string, string> = {}
    for (const field of preset.extra_fields) {
      vals[field.id] = field.default_value
    }
    extraValues.value = vals
  } else {
    extraValues.value = {}
  }
}

// When editing, populate from provider.extra
if (props.provider) {
  extraValues.value = { ...props.provider.extra }
} else {
  initExtraValues(kind.value)
}

// Test state
const testStatus = ref<'idle' | 'loading' | 'success' | 'error'>('idle')
const testMessage = ref('')
const fetchedModels = ref<string[]>(props.provider?.cached_models ?? [])

const currentPreset = computed(() =>
  engines.providerPresets.find(p => p.id === kind.value),
)

const visibleExtraFields = computed(() => {
  const preset = currentPreset.value
  if (!preset) return []
  const hidden = new Set(preset.hidden_fields ?? [])
  return preset.extra_fields.filter(field => !hidden.has(field.id))
})

const showUrl = computed(() => {
  if (kind.value === 'custom') return true
  return !(currentPreset.value?.hidden_fields?.includes('base_url'))
})

const showApiKey = computed(() =>
  !(currentPreset.value?.hidden_fields?.includes('api_key')),
)

const canTest = computed(() => {
  if (!showApiKey.value) return true
  return apiKey.value.trim().length > 0 || isEditing.value
})
const showInsecureToggle = computed(() => kind.value === 'custom')
const showCapabilities = computed(() => kind.value === 'custom')

const searchTerm = ref('')

const customOption = computed(() => ({ value: 'custom', label: t('provider.kind.custom') }))

const allOptions = computed(() => [
  customOption.value,
  ...engines.providerPresets.map(p => ({ value: p.id, label: p.display_name })),
])

const presetOptions = computed(() =>
  engines.providerPresets.map(p => ({ value: p.id, label: p.display_name })),
)

const filteredCustom = computed(() => {
  if (!searchTerm.value) return customOption.value
  const q = searchTerm.value.toLowerCase()
  return customOption.value.label.toLowerCase().includes(q) ? customOption.value : null
})

const filteredPresets = computed(() => {
  if (!searchTerm.value) return presetOptions.value
  const q = searchTerm.value.toLowerCase()
  return presetOptions.value.filter(o => o.label.toLowerCase().includes(q))
})

const presetDisplayName = computed(() => {
  const preset = engines.providerPresets.find(p => p.id === kind.value)
  return preset?.display_name ?? kind.value
})

function displayValue(val: unknown): string {
  if (typeof val !== 'string') return ''
  const opt = allOptions.value.find(o => o.value === val)
  return opt?.label ?? val
}

/** Resolve i18n label for an extra field: use i18n key if it exists, otherwise fallback to field.label. */
function fieldLabel(fieldId: string, fallback: string): string {
  const key = `provider.field.${fieldId}`
  return te(key) ? t(key) : fallback
}

watch(kind, (newKind) => {
  if (isEditing.value) return
  const preset = engines.providerPresets.find(p => p.id === newKind)
  if (preset) {
    name.value = preset.display_name
    url.value = preset.base_url
  } else {
    name.value = ''
    url.value = ''
  }
  // Reset test state and capabilities on kind change
  testStatus.value = 'idle'
  testMessage.value = ''
  fetchedModels.value = []
  supportsAsr.value = true
  supportsLlm.value = true
  // Reset extra values to preset defaults
  initExtraValues(newKind)
}, { immediate: true })

function onKindChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value === 'string') {
    kind.value = value
  }
}

function validate(): boolean {
  errors.value = {}
  if (!name.value.trim()) errors.value.name = t('validation.required')
  if (showUrl.value && !url.value.trim()) errors.value.url = t('validation.required')
  // Validate required extra fields
  for (const field of visibleExtraFields.value) {
    if (field.required && !(extraValues.value[field.id]?.trim())) {
      errors.value[field.id] = t('validation.required')
    }
  }
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
    allow_insecure: allowInsecure.value,
    cached_models: [],
    supports_asr: supportsAsr.value,
    supports_llm: supportsLlm.value,
    extra: { ...extraValues.value },
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
    testMessage.value = msg.length > 120 ? msg.slice(0, 120) + '\u2026' : msg
    fetchedModels.value = []
  }
}

function save() {
  if (!validate()) return

  const isCustom = kind.value === 'custom'
  const provider: Provider = {
    id: props.provider?.id ?? `provider-${kind.value}-${Date.now()}`,
    name: name.value.trim(),
    kind: kind.value,
    url: url.value.trim(),
    api_key: apiKey.value.trim(),
    allow_insecure: allowInsecure.value,
    cached_models: fetchedModels.value,
    supports_asr: isCustom ? supportsAsr.value : true,
    supports_llm: isCustom ? supportsLlm.value : true,
    extra: { ...extraValues.value },
  }

  emit('save', provider)
}
</script>

<template>
  <div class="space-y-4">
    <!-- Add mode: kind selector -->
    <div v-if="!isEditing" class="space-y-2">
      <Label class="text-xs text-muted-foreground">{{ t('provider.kind') }}</Label>
      <Combobox
        :model-value="kind"
        v-model:search-term="searchTerm"
        :reset-search-term-on-select="true"
        :reset-search-term-on-blur="true"
        @update:model-value="onKindChange"
      >
        <ComboboxAnchor class="flex h-9 w-full items-center rounded-md border border-input bg-transparent shadow-sm ring-offset-background focus-within:ring-1 focus-within:ring-ring">
          <ComboboxInput
            :display-value="displayValue"
            :placeholder="t('provider.searchPreset')"
            class="h-full w-full bg-transparent px-3 py-2 text-sm placeholder:text-muted-foreground focus:outline-none"
          />
          <ComboboxTrigger class="px-2 text-muted-foreground">
            <ChevronsUpDown class="w-4 h-4 opacity-50 shrink-0" />
          </ComboboxTrigger>
        </ComboboxAnchor>
        <ComboboxContent>
          <ComboboxEmpty>{{ t('provider.noResults') }}</ComboboxEmpty>
          <ComboboxItem v-if="filteredCustom" :value="filteredCustom.value">
            {{ filteredCustom.label }}
          </ComboboxItem>
          <Separator v-if="filteredCustom && filteredPresets.length" class="my-1" />
          <ComboboxItem
            v-for="option in filteredPresets"
            :key="option.value"
            :value="option.value"
          >
            {{ option.label }}
          </ComboboxItem>
        </ComboboxContent>
      </Combobox>
    </div>

    <div class="space-y-2">
      <div class="flex items-center justify-between">
        <Label class="text-xs text-muted-foreground">{{ t('provider.name') }}</Label>
        <span v-if="isEditing" class="text-xs px-1.5 py-0.5 rounded bg-muted text-muted-foreground">{{ presetDisplayName }}</span>
      </div>
      <Input v-model="name" class="h-9 text-sm" />
      <p v-if="errors.name" class="text-xs text-destructive">{{ errors.name }}</p>
    </div>

    <div v-if="showUrl" class="space-y-2">
      <Label class="text-xs text-muted-foreground">{{ t('provider.url') }}</Label>
      <Input v-model="url" class="h-9 text-sm" />
      <p v-if="errors.url" class="text-xs text-destructive">{{ errors.url }}</p>
    </div>

    <div v-if="showApiKey" class="space-y-2">
      <Label class="text-xs text-muted-foreground">{{ t('provider.apiKey') }}</Label>
      <div class="flex gap-2">
        <Input v-model="apiKey" type="password" :placeholder="isEditing ? t('provider.apiKeyKeep') : 'sk-...'" class="h-9 text-sm flex-1" />
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
    </div>

    <!-- Test button (standalone) when API key is hidden -->
    <div v-if="!showApiKey" class="space-y-2">
      <Button
        variant="outline"
        size="sm"
        class="w-full h-9"
        :disabled="!canTest || testStatus === 'loading'"
        @click="testConnection"
      >
        <Loader2 v-if="testStatus === 'loading'" class="w-3.5 h-3.5 animate-spin mr-2" />
        <template v-else>{{ t('provider.test') }}</template>
      </Button>
    </div>

    <!-- Test result -->
    <div v-if="testStatus === 'success'" class="flex items-center gap-1.5 text-xs text-green-600">
      <CheckCircle2 class="w-3.5 h-3.5" />
      <span>{{ testMessage }}</span>
    </div>
    <div v-if="testStatus === 'error'" class="flex items-start gap-1.5 rounded-md border border-destructive/30 bg-destructive/5 px-2.5 py-2 text-xs text-destructive">
      <XCircle class="w-3.5 h-3.5 shrink-0 mt-px" />
      <span>{{ testMessage }}</span>
    </div>

    <!-- Dynamic preset fields -->
    <div v-for="field in visibleExtraFields" :key="field.id" class="space-y-2">
      <Label class="text-xs text-muted-foreground">
        {{ fieldLabel(field.id, field.label) }}
        <span v-if="field.required" class="text-destructive">*</span>
      </Label>

      <!-- Text / Password input -->
      <Input
        v-if="field.field_type !== 'select'"
        :model-value="extraValues[field.id] ?? ''"
        @update:model-value="v => extraValues[field.id] = String(v)"
        :type="field.field_type"
        :placeholder="field.placeholder"
        class="h-9 text-sm"
      />

      <!-- Select dropdown -->
      <Select
        v-else
        :model-value="extraValues[field.id] ?? ''"
        @update:model-value="v => extraValues[field.id] = String(v)"
      >
        <SelectTrigger class="w-full h-9 text-sm">
          <SelectValue />
        </SelectTrigger>
        <SelectContent class="max-h-[45vh]">
          <SelectItem
            v-for="[value, label] in field.options"
            :key="value"
            :value="value"
          >{{ label }}</SelectItem>
        </SelectContent>
      </Select>

      <p v-if="errors[field.id]" class="text-xs text-destructive">{{ errors[field.id] }}</p>
    </div>

    <!-- Capabilities — only for Custom providers -->
    <div v-if="showCapabilities" class="space-y-2">
      <Label class="text-xs text-muted-foreground">{{ t('provider.capabilities') }}</Label>
      <div class="flex gap-4">
        <label class="flex items-center gap-2 text-sm cursor-pointer">
          <Switch :checked="supportsAsr" @update:checked="supportsAsr = $event" />
          {{ t('provider.capabilities.asr') }}
        </label>
        <label class="flex items-center gap-2 text-sm cursor-pointer">
          <Switch :checked="supportsLlm" @update:checked="supportsLlm = $event" />
          {{ t('provider.capabilities.llm') }}
        </label>
      </div>
    </div>

    <!-- Allow insecure (HTTP) — only for Custom providers -->
    <div v-if="showInsecureToggle" class="flex items-center justify-between gap-3 rounded-md border border-amber-500/20 bg-amber-500/5 px-3 py-2.5">
      <div class="flex items-start gap-2 min-w-0">
        <ShieldAlert class="w-4 h-4 text-amber-500 shrink-0 mt-0.5" />
        <div class="min-w-0">
          <div class="text-xs font-medium">{{ t('provider.allowInsecure') }}</div>
          <div class="text-[11px] text-muted-foreground">{{ t('provider.allowInsecureDesc') }}</div>
        </div>
      </div>
      <Switch :checked="allowInsecure" @update:checked="allowInsecure = $event" />
    </div>

    <div class="flex justify-end gap-2 pt-2">
      <Button variant="outline" size="sm" @click="emit('cancel')">{{ t('modelManager.cancel') }}</Button>
      <Button size="sm" @click="save">{{ t('modelManager.save') }}</Button>
    </div>
  </div>
</template>
