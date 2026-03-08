import { createI18n } from 'vue-i18n'
import enRaw from './en.json'
import frRaw from './fr.json'

// Strip _version (used by rust-i18n, not needed by vue-i18n)
const { _version: _v1, ...en } = enRaw
const { _version: _v2, ...fr } = frRaw

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  fallbackLocale: 'en',
  messages: { en, fr },
})

export default i18n
