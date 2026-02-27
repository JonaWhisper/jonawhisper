import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createRouter, createWebHistory } from 'vue-router'
import { createI18n } from 'vue-i18n'
import App from './App.vue'
import en from './i18n/en.json'
import fr from './i18n/fr.json'
import './assets/main.css'

const i18n = createI18n({
  legacy: false,
  locale: navigator.language.startsWith('fr') ? 'fr' : 'en',
  fallbackLocale: 'en',
  messages: { en, fr },
})

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/', redirect: '/pill' },
    { path: '/pill', component: () => import('./components/FloatingPill.vue') },
    { path: '/model-manager', component: () => import('./components/ModelManager.vue') },
    { path: '/setup', component: () => import('./components/SetupWizard.vue') },
  ],
})

const pinia = createPinia()

const app = createApp(App)
app.use(pinia)
app.use(router)
app.use(i18n)
app.mount('#app')
