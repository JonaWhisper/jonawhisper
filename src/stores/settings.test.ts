import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

const mockInvoke = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}))
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn(),
}))

import { useSettingsStore } from './settings'
import type { SettingsPayload } from './types'

function makePayload(overrides: Partial<SettingsPayload> = {}): SettingsPayload {
  return {
    app_locale: 'auto',
    hallucination_filter_enabled: true,
    hotkey: 'right_command',
    cancel_shortcut: 'escape',
    recording_mode: 'push_to_talk',
    selected_input_device_uid: null,
    selected_model_id: 'whisper:large-v3-turbo-q8',
    selected_language: 'auto',
    text_cleanup_enabled: false,
    cleanup_model_id: '',
    llm_provider_id: '',
    llm_model: '',
    asr_cloud_model: 'whisper-1',
    gpu_mode: 'auto',
    llm_max_tokens: 4096,
    audio_ducking_enabled: false,
    audio_ducking_level: 0.2,
    vad_enabled: true,
    punctuation_model_id: '',
    disfluency_removal_enabled: true,
    itn_enabled: true,
    spellcheck_enabled: false,
    theme: 'system',
    log_level: 'info',
    log_retention: 'previous',
    ...overrides,
  }
}

describe('settings store', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockInvoke.mockReset()
  })

  describe('applySettingLocally', () => {
    it('coerces bool string "true" for hallucination_filter_enabled', () => {
      const store = useSettingsStore()
      store.applySettingLocally('hallucination_filter_enabled', 'true')
      expect(store.hallucinationFilterEnabled).toBe(true)
    })

    it('coerces bool string "false" for hallucination_filter_enabled', () => {
      const store = useSettingsStore()
      store.applySettingLocally('hallucination_filter_enabled', 'false')
      expect(store.hallucinationFilterEnabled).toBe(false)
    })

    it('parses llm_max_tokens as int with fallback 4096', () => {
      const store = useSettingsStore()
      store.applySettingLocally('llm_max_tokens', '2048')
      expect(store.llmMaxTokens).toBe(2048)

      store.applySettingLocally('llm_max_tokens', 'not_a_number')
      expect(store.llmMaxTokens).toBe(4096)
    })

    it('parses audio_ducking_level as float with fallback 0.8', () => {
      const store = useSettingsStore()
      store.applySettingLocally('audio_ducking_level', '0.5')
      expect(store.audioDuckingLevel).toBe(0.5)

      store.applySettingLocally('audio_ducking_level', 'invalid')
      expect(store.audioDuckingLevel).toBe(0.8)
    })

    it('maps empty selected_input_device_uid to null', () => {
      const store = useSettingsStore()
      store.applySettingLocally('selected_input_device_uid', 'some-uid')
      expect(store.selectedInputDeviceUid).toBe('some-uid')

      store.applySettingLocally('selected_input_device_uid', '')
      expect(store.selectedInputDeviceUid).toBeNull()
    })
  })

  describe('getSettingValue', () => {
    it('returns string representation for bool settings', () => {
      const store = useSettingsStore()
      store.hallucinationFilterEnabled = true
      expect(store.getSettingValue('hallucination_filter_enabled')).toBe('true')

      store.hallucinationFilterEnabled = false
      expect(store.getSettingValue('hallucination_filter_enabled')).toBe('false')
    })

    it('returns string for numeric settings', () => {
      const store = useSettingsStore()
      store.llmMaxTokens = 8192
      expect(store.getSettingValue('llm_max_tokens')).toBe('8192')
    })

    it('returns empty string for null device uid', () => {
      const store = useSettingsStore()
      store.selectedInputDeviceUid = null
      expect(store.getSettingValue('selected_input_device_uid')).toBe('')
    })

    it('returns empty string for unknown key', () => {
      const store = useSettingsStore()
      expect(store.getSettingValue('nonexistent_key')).toBe('')
    })
  })

  describe('setSetting', () => {
    it('calls invoke and applies locally', async () => {
      const store = useSettingsStore()
      mockInvoke.mockResolvedValueOnce(undefined)

      await store.setSetting('selected_language', 'fr')

      expect(store.selectedLanguage).toBe('fr')
      expect(mockInvoke).toHaveBeenCalledWith('set_setting', { key: 'selected_language', value: 'fr' })
    })

    it('rolls back on invoke error', async () => {
      const store = useSettingsStore()
      store.selectedLanguage = 'en'
      mockInvoke.mockRejectedValueOnce(new Error('backend error'))

      await store.setSetting('selected_language', 'fr')

      expect(store.selectedLanguage).toBe('en')
    })
  })

  describe('fetchSettings', () => {
    it('populates all fields from backend payload', async () => {
      const store = useSettingsStore()
      mockInvoke.mockResolvedValueOnce(makePayload({
        app_locale: 'fr',
        selected_model_id: 'canary:1b',
        vad_enabled: false,
        theme: 'dark',
        llm_max_tokens: 1024,
      }))

      await store.fetchSettings()

      expect(store.appLocale).toBe('fr')
      expect(store.selectedModelId).toBe('canary:1b')
      expect(store.vadEnabled).toBe(false)
      expect(store.theme).toBe('dark')
      expect(store.llmMaxTokens).toBe(1024)
    })

    it('uses defaults when optional fields are absent', async () => {
      const store = useSettingsStore()
      // Simulate a payload with undefined optional fields
      const partial = makePayload()
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      delete (partial as any).text_cleanup_enabled
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      delete (partial as any).gpu_mode
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      delete (partial as any).theme
      mockInvoke.mockResolvedValueOnce(partial)

      await store.fetchSettings()

      expect(store.textCleanupEnabled).toBe(false)
      expect(store.gpuMode).toBe('auto')
      expect(store.theme).toBe('system')
    })
  })
})
