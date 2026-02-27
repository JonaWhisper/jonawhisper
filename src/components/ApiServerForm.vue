<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useAppStore, type ApiServerConfig } from '@/stores/app'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog'

const { t } = useI18n()
const store = useAppStore()

const emit = defineEmits<{
  close: []
}>()

const open = ref(true)
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

function handleOpenChange(value: boolean) {
  if (!value) emit('close')
}
</script>

<template>
  <Dialog :open="open" @update:open="handleOpenChange">
    <DialogContent class="sm:max-w-md">
      <DialogHeader>
        <DialogTitle>{{ t('modelManager.addApiServer') }}</DialogTitle>
      </DialogHeader>

      <div class="space-y-4">
        <div class="space-y-2">
          <Label>{{ t('apiServer.name') }}</Label>
          <Input v-model="name" :placeholder="t('apiServer.namePlaceholder')" />
          <p v-if="errors.name" class="text-xs text-destructive">{{ errors.name }}</p>
        </div>

        <div class="space-y-2">
          <Label>{{ t('apiServer.url') }}</Label>
          <Input v-model="url" :placeholder="t('apiServer.urlPlaceholder')" />
          <p v-if="errors.url" class="text-xs text-destructive">{{ errors.url }}</p>
        </div>

        <div class="space-y-2">
          <Label>{{ t('apiServer.apiKey') }}</Label>
          <Input v-model="apiKey" type="password" :placeholder="t('apiServer.apiKeyPlaceholder')" />
        </div>

        <div class="space-y-2">
          <Label>{{ t('apiServer.model') }}</Label>
          <Input v-model="model" :placeholder="t('apiServer.modelPlaceholder')" />
          <p v-if="errors.model" class="text-xs text-destructive">{{ errors.model }}</p>
        </div>
      </div>

      <DialogFooter>
        <Button variant="outline" @click="emit('close')">{{ t('modelManager.cancel') }}</Button>
        <Button @click="save">{{ t('modelManager.save') }}</Button>
      </DialogFooter>
    </DialogContent>
  </Dialog>
</template>
