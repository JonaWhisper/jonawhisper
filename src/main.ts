import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import App from './App.vue'
import router from './router'
import enRaw from './i18n/en.json'
import frRaw from './i18n/fr.json'
import './assets/main.css'

// Strip _version (used by rust-i18n, not needed by vue-i18n)
const { _version: _v1, ...en } = enRaw
const { _version: _v2, ...fr } = frRaw

export const i18n = createI18n({
  legacy: false,
  locale: 'en',
  fallbackLocale: 'en',
  messages: { en, fr },
})

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
