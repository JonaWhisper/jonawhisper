<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEnginesStore } from '@/stores/engines'
import type { Provider } from '@/stores/types'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import ConfirmDialog from '@/components/ConfirmDialog.vue'
import ProviderForm from '@/components/ProviderForm.vue'
import {
  Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription,
} from '@/components/ui/dialog'
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from '@/components/ui/tooltip'
import { Plus, Pencil, Trash2 } from 'lucide-vue-next'

const { t } = useI18n()
const engines = useEnginesStore()

// Add dialog
const showAddDialog = ref(false)
const addFormKey = ref(0)

// Edit dialog
const showEditDialog = ref(false)
const editingProvider = ref<Provider | null>(null)

// Delete confirm
const showRemoveConfirm = ref(false)
const removeTarget = ref<Provider | null>(null)

function openAddDialog() {
  addFormKey.value++
  showAddDialog.value = true
}

async function saveNewProvider(provider: Provider) {
  await engines.addProvider(provider)
  showAddDialog.value = false
}

function openEditDialog(provider: Provider) {
  editingProvider.value = { ...provider }
  showEditDialog.value = true
}

async function saveEditedProvider(provider: Provider) {
  await engines.updateProvider(provider)
  showEditDialog.value = false
  editingProvider.value = null
}

function requestRemoveProvider(provider: Provider) {
  removeTarget.value = provider
  showRemoveConfirm.value = true
}

async function confirmRemoveProvider() {
  if (removeTarget.value) {
    await engines.removeProvider(removeTarget.value.id)
  }
  showRemoveConfirm.value = false
  removeTarget.value = null
}

function providerInitial(provider: Provider): string {
  return provider.name.charAt(0).toUpperCase()
}

const kindColors: Record<string, string> = {
  OpenAI: 'bg-emerald-500',
  Anthropic: 'bg-orange-500',
  Groq: 'bg-purple-500',
  Cerebras: 'bg-blue-500',
  Gemini: 'bg-sky-500',
  Mistral: 'bg-indigo-500',
  Fireworks: 'bg-red-500',
  Together: 'bg-teal-500',
  DeepSeek: 'bg-cyan-500',
  Custom: 'bg-zinc-500',
}
</script>

<template>
  <div class="space-y-3">
    <!-- Header with add button -->
    <div class="flex items-center justify-between">
      <span class="text-sm text-muted-foreground">
        {{ engines.providers.length === 0 ? t('settings.providers.empty') : '' }}
      </span>
      <TooltipProvider :delay-duration="300">
        <Tooltip>
          <TooltipTrigger as-child>
            <Button variant="outline" size="icon" class="h-7 w-7" @click="openAddDialog">
              <Plus class="w-4 h-4" />
            </Button>
          </TooltipTrigger>
          <TooltipContent side="bottom" :side-offset="4">{{ t('settings.providers.add') }}</TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </div>

    <!-- Provider list -->
    <div v-for="provider in engines.providers" :key="provider.id" class="rounded-md border border-border">
      <div class="flex items-center gap-3 px-3 py-2">
        <div
          :class="['flex items-center justify-center w-8 h-8 rounded-md text-white text-sm font-bold shrink-0', kindColors[provider.kind] ?? 'bg-zinc-500']"
        >
          {{ providerInitial(provider) }}
        </div>
        <div class="flex-1 min-w-0">
          <div class="text-sm font-medium truncate">{{ provider.name }}</div>
          <div class="text-xs text-muted-foreground truncate">
            {{ provider.api_key ? '••••' + provider.api_key.slice(-4) : '' }}
          </div>
        </div>
        <Badge variant="secondary" class="text-[10px] px-1.5 py-0 shrink-0">{{ provider.kind }}</Badge>
        <Button variant="ghost" size="icon" class="h-7 w-7 shrink-0" @click="openEditDialog(provider)">
          <Pencil class="w-3.5 h-3.5" />
        </Button>
        <Button variant="ghost" size="icon" class="h-7 w-7 shrink-0 text-destructive hover:text-destructive" @click="requestRemoveProvider(provider)">
          <Trash2 class="w-3.5 h-3.5" />
        </Button>
      </div>
    </div>

    <!-- Add provider dialog -->
    <Dialog v-model:open="showAddDialog">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ t('settings.providers.add') }}</DialogTitle>
          <DialogDescription></DialogDescription>
        </DialogHeader>
        <ProviderForm
          :key="addFormKey"
          @save="saveNewProvider"
          @cancel="showAddDialog = false"
        />
      </DialogContent>
    </Dialog>

    <!-- Edit provider dialog -->
    <Dialog v-model:open="showEditDialog">
      <DialogContent class="sm:max-w-md">
        <DialogHeader>
          <DialogTitle>{{ editingProvider?.name }}</DialogTitle>
          <DialogDescription></DialogDescription>
        </DialogHeader>
        <ProviderForm
          v-if="editingProvider"
          :provider="editingProvider"
          @save="saveEditedProvider"
          @cancel="showEditDialog = false"
        />
      </DialogContent>
    </Dialog>

    <!-- Remove confirmation -->
    <ConfirmDialog
      v-model:open="showRemoveConfirm"
      :title="t('settings.providers.removeConfirm')"
      :description="t('settings.providers.removeConfirmDesc')"
      :confirm-label="t('modelManager.delete')"
      @confirm="confirmRemoveProvider"
    />
  </div>
</template>
