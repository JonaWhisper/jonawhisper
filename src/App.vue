<script setup lang="ts">
import { onMounted } from 'vue'
import { useAppStore } from './stores/app'

const store = useAppStore()

onMounted(async () => {
  // Detect dark mode
  if (window.matchMedia('(prefers-color-scheme: dark)').matches) {
    document.documentElement.classList.add('dark')
  }
  window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
    document.documentElement.classList.toggle('dark', e.matches)
  })

  await store.init()
})
</script>

<template>
  <router-view />
</template>
