import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { invoke } from '@tauri-apps/api/core'
import App from './App.vue'
import router from './router'
import i18n from './i18n'
import './assets/main.css'

// Load effective locale from Rust (single source of truth)
invoke<string>('get_system_locale').then((locale) => {
  i18n.global.locale.value = locale as 'fr' | 'en'
}).catch(() => {
  // Fallback to browser language if backend not ready
  if (navigator.language.startsWith('fr')) {
    i18n.global.locale.value = 'fr'
  }
})

const pinia = createPinia()

const app = createApp(App)
app.use(pinia)
app.use(router)
app.use(i18n)
app.mount('#app')
