<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore, type ApiServerConfig } from '../stores/app'

const { t } = useI18n()
const store = useAppStore()

const emit = defineEmits<{
  close: []
}>()

const name = ref('')
const url = ref('')
const apiKey = ref('')
const model = ref('')
const errors = ref<Record<string, string>>({})

function validate(): boolean {
  errors.value = {}
  if (!name.value.trim()) errors.value.name = 'Required'
  if (!url.value.trim()) errors.value.url = 'Required'
  if (!model.value.trim()) errors.value.model = 'Required'
  return Object.keys(errors.value).length === 0
}

async function save() {
  if (!validate()) return

  const config: ApiServerConfig = {
    id: `api-${Date.now()}`,
    name: name.value.trim(),
    url: url.value.trim(),
    api_key: apiKey.value.trim(),
    model: model.value.trim(),
  }

  await store.addApiServer(config)
  emit('close')
}
</script>

<template>
  <div class="fixed inset-0 bg-black/50 flex items-center justify-center z-50">
    <div class="bg-background border border-border rounded-lg p-6 w-full max-w-md mx-4 shadow-xl">
      <h3 class="text-lg font-semibold mb-4">{{ t('modelManager.addApiServer') }}</h3>

      <div class="space-y-4">
        <div>
          <label class="block text-sm font-medium mb-1">{{ t('apiServer.name') }}</label>
          <input
            v-model="name"
            :placeholder="t('apiServer.namePlaceholder')"
            class="w-full px-3 py-2 text-sm rounded-md border border-input bg-background focus:outline-none focus:ring-2 focus:ring-ring"
          />
          <p v-if="errors.name" class="text-xs text-destructive mt-1">{{ errors.name }}</p>
        </div>

        <div>
          <label class="block text-sm font-medium mb-1">{{ t('apiServer.url') }}</label>
          <input
            v-model="url"
            :placeholder="t('apiServer.urlPlaceholder')"
            class="w-full px-3 py-2 text-sm rounded-md border border-input bg-background focus:outline-none focus:ring-2 focus:ring-ring"
          />
          <p v-if="errors.url" class="text-xs text-destructive mt-1">{{ errors.url }}</p>
        </div>

        <div>
          <label class="block text-sm font-medium mb-1">{{ t('apiServer.apiKey') }}</label>
          <input
            v-model="apiKey"
            type="password"
            :placeholder="t('apiServer.apiKeyPlaceholder')"
            class="w-full px-3 py-2 text-sm rounded-md border border-input bg-background focus:outline-none focus:ring-2 focus:ring-ring"
          />
        </div>

        <div>
          <label class="block text-sm font-medium mb-1">{{ t('apiServer.model') }}</label>
          <input
            v-model="model"
            :placeholder="t('apiServer.modelPlaceholder')"
            class="w-full px-3 py-2 text-sm rounded-md border border-input bg-background focus:outline-none focus:ring-2 focus:ring-ring"
          />
          <p v-if="errors.model" class="text-xs text-destructive mt-1">{{ errors.model }}</p>
        </div>
      </div>

      <div class="flex gap-2 justify-end mt-6">
        <button
          @click="emit('close')"
          class="px-4 py-2 text-sm rounded-md border border-border hover:bg-accent transition-colors"
        >
          {{ t('modelManager.cancel') }}
        </button>
        <button
          @click="save"
          class="px-4 py-2 text-sm rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
        >
          {{ t('modelManager.save') }}
        </button>
      </div>
    </div>
  </div>
</template>
