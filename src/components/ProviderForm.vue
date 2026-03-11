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
import { Switch } from '@/components/ui/switch'
import { Loader2, CheckCircle2, XCircle, ChevronsUpDown, ShieldAlert } from 'lucide-vue-next'

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

const kind = ref(props.provider?.kind ?? (engines.providerPresets[0]?.id ?? 'openai-compatible'))
const name = ref(props.provider?.name ?? '')
const errors = ref<Record<string, string>>({})
const extraValues = ref<Record<string, string>>({})

// Initialize extra values from preset defaults
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

/** Map top-level Provider fields back into extraValues for editing. */
function applyEditMappings(provider: Provider) {
  Object.assign(extraValues.value, provider.extra)
  if (provider.url && !extraValues.value['base_url']) {
    extraValues.value['base_url'] = provider.url
  }
  if ('supports_asr' in provider) {
    extraValues.value['supports_asr'] = String(provider.supports_asr)
  }
  if ('supports_llm' in provider) {
    extraValues.value['supports_llm'] = String(provider.supports_llm)
  }
  if ('allow_insecure' in provider) {
    extraValues.value['allow_insecure'] = String(provider.allow_insecure)
  }
}

// When editing, start with preset defaults then overlay existing values.
if (props.provider) {
  initExtraValues(kind.value)
  applyEditMappings(props.provider)
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

const inputFields = computed(() => visibleExtraFields.value.filter(f => f.field_type !== 'toggle'))
const hasCapabilities = computed(() => visibleExtraFields.value.some(f => f.id === 'supports_asr' || f.id === 'supports_llm'))
const hasInsecureToggle = computed(() => visibleExtraFields.value.some(f => f.id === 'allow_insecure'))

const canTest = computed(() => {
  for (const field of visibleExtraFields.value) {
    if (!field.required) continue
    const val = extraValues.value[field.id]?.trim()
    if (!val) {
      if (field.sensitive && isEditing.value) continue
      return false
    }
  }
  // Custom providers need a URL to test
  const baseUrl = extraValues.value['base_url']?.trim()
  if (visibleExtraFields.value.some(f => f.id === 'base_url') && !baseUrl) return false
  return true
})

const searchTerm = ref('')

const allOptions = computed(() =>
  engines.providerPresets.map(p => ({ value: p.id, label: p.display_name })),
)

const filteredOptions = computed(() => {
  if (!searchTerm.value) return allOptions.value
  const q = searchTerm.value.toLowerCase()
  return allOptions.value.filter(o => o.label.toLowerCase().includes(q))
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
  } else {
    name.value = ''
  }
  // Reset test state on kind change
  testStatus.value = 'idle'
  testMessage.value = ''
  fetchedModels.value = []
  // Reset extra values to preset defaults
  initExtraValues(newKind)
}, { immediate: true })

// Re-apply preset defaults when presets arrive asynchronously
let presetsInitialized = false
watch(() => engines.providerPresets, (presets) => {
  if (!presets.length || presetsInitialized) return
  presetsInitialized = true
  initExtraValues(kind.value)
  if (props.provider) {
    applyEditMappings(props.provider)
  }
})

function onKindChange(value: string | number | bigint | Record<string, unknown> | null) {
  if (typeof value === 'string') {
    kind.value = value
  }
}

function validate(): boolean {
  errors.value = {}
  if (!name.value.trim()) errors.value.name = t('validation.required')
  // Validate required extra fields (sensitive fields skip validation when editing — empty = keep existing)
  for (const field of visibleExtraFields.value) {
    if (field.required && !(extraValues.value[field.id]?.trim())) {
      if (field.sensitive && isEditing.value) continue
      errors.value[field.id] = t('validation.required')
    }
  }
  // Enforce HTTPS for custom base_url unless allow_insecure is enabled
  const baseUrl = extraValues.value['base_url']?.trim() ?? ''
  if (baseUrl && baseUrl.startsWith('http://') && extraValues.value['allow_insecure'] !== 'true' && !errors.value['base_url']) {
    errors.value['base_url'] = t('validation.httpsRequired')
  }
  return Object.keys(errors.value).length === 0
}

/** Fields that map to top-level Provider properties (not stored in extra). */
const TOP_LEVEL_FIELDS = new Set(['api_key', 'base_url', 'supports_asr', 'supports_llm', 'allow_insecure'])

function buildProvider(): Provider {
  const preset = currentPreset.value
  const ev = extraValues.value
  return {
    id: props.provider?.id ?? `provider-${kind.value}-${Date.now()}`,
    name: name.value.trim(),
    kind: kind.value,
    url: ev['base_url']?.trim() ?? preset?.base_url ?? '',
    api_key: ev['api_key']?.trim() ?? '',
    allow_insecure: ev['allow_insecure'] === 'true',
    cached_models: fetchedModels.value,
    supports_asr: ev['supports_asr'] != null ? ev['supports_asr'] === 'true' : (preset?.supports_asr ?? true),
    supports_llm: ev['supports_llm'] != null ? ev['supports_llm'] === 'true' : (preset?.supports_llm ?? true),
    extra: Object.fromEntries(
      Object.entries(ev).filter(([k]) => !TOP_LEVEL_FIELDS.has(k)),
    ),
  }
}

async function testConnection() {
  testStatus.value = 'loading'
  testMessage.value = ''

  const tempProvider = { ...buildProvider(), id: props.provider?.id ?? 'temp', cached_models: [] as string[] }

  try {
    const models = await invoke<string[]>('fetch_provider_models', { provider: tempProvider })
    fetchedModels.value = models
    testStatus.value = 'success'
    testMessage.value = t('provider.testSuccess', [models.length])
  } catch (e) {
    testStatus.value = 'error'
    const msg = String(e)
    testMessage.value = msg.length > 120 ? msg.slice(0, 120) + '\u2026' : msg
    fetchedModels.value = []
  }
}

function save() {
  if (!validate()) return
  emit('save', buildProvider())
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
        :open-on-focus="true"
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
          <ComboboxItem
            v-for="option in filteredOptions"
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

    <!-- Test result -->
    <div v-if="testStatus === 'success'" class="flex items-center gap-1.5 text-xs text-green-600">
      <CheckCircle2 class="w-3.5 h-3.5" />
      <span>{{ testMessage }}</span>
    </div>
    <div v-if="testStatus === 'error'" class="flex items-start gap-1.5 rounded-md border border-destructive/30 bg-destructive/5 px-2.5 py-2 text-xs text-destructive">
      <XCircle class="w-3.5 h-3.5 shrink-0 mt-px" />
      <span>{{ testMessage }}</span>
    </div>

    <!-- Dynamic input fields (text, password, select) -->
    <div v-for="field in inputFields" :key="field.id" class="space-y-2">
      <Label class="text-xs text-muted-foreground">
        {{ fieldLabel(field.id, field.label) }}
        <span v-if="field.required" class="text-destructive">*</span>
      </Label>

      <!-- Text / Password input -->
      <Input
        v-if="field.field_type === 'text' || field.field_type === 'password'"
        :model-value="extraValues[field.id] ?? ''"
        @update:model-value="v => extraValues[field.id] = String(v)"
        :type="field.field_type"
        :placeholder="isEditing && field.sensitive ? t('provider.apiKeyKeep') : field.placeholder"
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

    <!-- Capabilities -->
    <div v-if="hasCapabilities" class="space-y-2">
      <Label class="text-xs text-muted-foreground">{{ t('provider.capabilities') }}</Label>
      <div class="flex gap-4">
        <label v-if="visibleExtraFields.some(f => f.id === 'supports_asr')" class="flex items-center gap-2 text-sm cursor-pointer">
          <Switch
            :checked="extraValues['supports_asr'] === 'true'"
            @update:checked="(v: boolean) => extraValues['supports_asr'] = String(v)"
          />
          {{ t('provider.capabilities.asr') }}
        </label>
        <label v-if="visibleExtraFields.some(f => f.id === 'supports_llm')" class="flex items-center gap-2 text-sm cursor-pointer">
          <Switch
            :checked="extraValues['supports_llm'] === 'true'"
            @update:checked="(v: boolean) => extraValues['supports_llm'] = String(v)"
          />
          {{ t('provider.capabilities.llm') }}
        </label>
      </div>
    </div>

    <!-- Allow insecure -->
    <div v-if="hasInsecureToggle" class="flex items-center justify-between gap-3 rounded-md border border-amber-500/20 bg-amber-500/5 px-3 py-2.5">
      <div class="flex items-start gap-2 min-w-0">
        <ShieldAlert class="w-4 h-4 text-amber-500 shrink-0 mt-0.5" />
        <div class="min-w-0">
          <div class="text-xs font-medium">{{ t('provider.allowInsecure') }}</div>
          <div class="text-[11px] text-muted-foreground">{{ t('provider.allowInsecureDesc') }}</div>
        </div>
      </div>
      <Switch
        :checked="extraValues['allow_insecure'] === 'true'"
        @update:checked="(v: boolean) => extraValues['allow_insecure'] = String(v)"
      />
    </div>

    <div class="flex justify-end gap-2 pt-2">
      <Button
        variant="outline"
        size="sm"
        :disabled="!canTest || testStatus === 'loading'"
        @click="testConnection"
      >
        <Loader2 v-if="testStatus === 'loading'" class="w-3.5 h-3.5 animate-spin" />
        <template v-else>{{ t('provider.test') }}</template>
      </Button>
      <div class="flex-1" />
      <Button variant="outline" size="sm" @click="emit('cancel')">{{ t('modelManager.cancel') }}</Button>
      <Button size="sm" @click="save">{{ t('modelManager.save') }}</Button>
    </div>
  </div>
</template>
