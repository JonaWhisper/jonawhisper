import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createI18n } from 'vue-i18n'
import { invoke } from '@tauri-apps/api/core'
import App from './App.vue'
import router from './router'
import en from './i18n/en.json'
import fr from './i18n/fr.json'
import './assets/main.css'

function systemLocale(): string {
  return navigator.language.startsWith('fr') ? 'fr' : 'en'
}

export const i18n = createI18n({
  legacy: false,
  locale: systemLocale(),
  fallbackLocale: 'en',
  messages: { en, fr },
})

// Load locale preference from backend (non-blocking)
invoke<{ app_locale: string }>('get_settings').then((settings) => {
  if (settings.app_locale && settings.app_locale !== 'auto') {
    i18n.global.locale.value = settings.app_locale as 'fr' | 'en'
  }
}).catch(() => {
  // Settings not available yet (e.g. during setup), use system locale
})

const pinia = createPinia()

const app = createApp(App)
app.use(pinia)
app.use(router)
app.use(i18n)
app.mount('#app')
