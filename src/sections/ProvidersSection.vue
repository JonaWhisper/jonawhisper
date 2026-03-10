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
import { Plus, Pencil, X } from 'lucide-vue-next'

const { t } = useI18n()
const engines = useEnginesStore()

const showAddDialog = ref(false)
const addFormKey = ref(0)
const showEditDialog = ref(false)
const editingProvider = ref<Provider | null>(null)
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

const CUSTOM_GRADIENT = 'linear-gradient(135deg, #636366, #48484a)'

function providerGradient(provider: Provider): string {
  const preset = engines.providerPresets.find(p => p.id === provider.kind)
  return preset?.gradient ?? CUSTOM_GRADIENT
}
</script>

<template>
  <div>
    <!-- Header: title + add button -->
    <div class="flex items-center justify-between mb-4">
      <div class="text-[20px] font-bold tracking-[-0.02em]">{{ t('panel.providers') }}</div>
      <TooltipProvider :delay-duration="300">
        <Tooltip>
          <TooltipTrigger as-child>
            <button
              class="w-7 h-7 flex items-center justify-center rounded-md border-none cursor-pointer transition-colors bg-sidebar-hover-bg text-muted-foreground hover:text-foreground"
              @click="openAddDialog"
            >
              <Plus class="w-4 h-4" />
            </button>
          </TooltipTrigger>
          <TooltipContent side="bottom" :side-offset="4">{{ t('settings.providers.add') }}</TooltipContent>
        </Tooltip>
      </TooltipProvider>
    </div>

    <!-- Provider card -->
    <div class="bg-panel-card-bg backdrop-blur border-[0.5px] border-panel-card-border rounded-xl shadow-panel-card p-[14px_16px] mb-2.5">
      <div class="text-[11px] font-semibold uppercase tracking-[0.04em] text-muted-foreground mb-2.5">{{ t('settings.providers.add') }}</div>

      <!-- Empty state -->
      <div v-if="engines.providers.length === 0" class="text-xs text-muted-foreground py-2">
        {{ t('settings.providers.empty') }}
      </div>

      <!-- Provider rows -->
      <div v-for="provider in engines.providers" :key="provider.id" class="flex items-center gap-3 py-2.5 [&+&]:border-t-[0.5px] [&+&]:border-panel-divider">
        <div
          class="flex items-center justify-center w-8 h-8 rounded-lg text-white text-base font-bold shrink-0"
          :style="{ background: providerGradient(provider) }"
        >
          {{ providerInitial(provider) }}
        </div>
        <div class="flex-1 min-w-0">
          <div class="flex items-center gap-1.5">
            <span class="text-[13px] font-semibold truncate">{{ provider.name }}</span>
            <Badge v-if="provider.supports_asr" variant="outline" class="text-[9px] px-1 py-0 shrink-0">ASR</Badge>
            <Badge v-if="provider.supports_llm" variant="outline" class="text-[9px] px-1 py-0 shrink-0">LLM</Badge>
          </div>
          <div class="text-[11px] text-muted-foreground truncate">
            {{ provider.api_key || '' }}
          </div>
        </div>
        <div class="flex gap-1 shrink-0">
          <Button variant="outline" size="icon" class="h-7 w-7" :aria-label="t('aria.edit')" @click="openEditDialog(provider)">
            <Pencil class="w-3.5 h-3.5" />
          </Button>
          <Button variant="destructive" size="icon" class="h-7 w-7" :aria-label="t('aria.delete')" @click="requestRemoveProvider(provider)">
            <X class="w-3.5 h-3.5" />
          </Button>
        </div>
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
