<script setup lang="ts">
import { onMounted } from 'vue'
import { useAppStore } from './stores/app'
import { useTheme } from './composables/useTheme'

const store = useAppStore()

useTheme()

onMounted(async () => {
  await store.init()
})
</script>

<template>
  <router-view />
</template>
