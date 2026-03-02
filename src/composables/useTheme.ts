import { watch, onUnmounted } from 'vue'
import { useSettingsStore } from '@/stores/settings'

function applyTheme(theme: string) {
  const root = document.documentElement
  if (theme === 'dark') {
    root.classList.add('dark')
  } else if (theme === 'light') {
    root.classList.remove('dark')
  } else {
    // system
    root.classList.toggle('dark', window.matchMedia('(prefers-color-scheme: dark)').matches)
  }
}

export function useTheme() {
  const settings = useSettingsStore()
  const mq = window.matchMedia('(prefers-color-scheme: dark)')

  function onSystemChange() {
    if (settings.theme === 'system') {
      applyTheme('system')
    }
  }

  mq.addEventListener('change', onSystemChange)
  onUnmounted(() => mq.removeEventListener('change', onSystemChange))

  // React to theme setting changes
  watch(() => settings.theme, (v) => applyTheme(v), { immediate: true })
}
