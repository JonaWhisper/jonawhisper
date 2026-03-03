import { defineSetupVue3 } from '@histoire/plugin-vue'
import { createPinia } from 'pinia'
import { createI18n } from 'vue-i18n'
import '../assets/main.css'

import enRaw from '../i18n/en.json'
import frRaw from '../i18n/fr.json'

const { _version: _v1, ...en } = enRaw
const { _version: _v2, ...fr } = frRaw

export const setupVue3 = defineSetupVue3(({ app }) => {
  const pinia = createPinia()
  const i18n = createI18n({
    legacy: false,
    locale: 'en',
    fallbackLocale: 'en',
    messages: { en, fr },
  })

  app.use(pinia)
  app.use(i18n)
})
