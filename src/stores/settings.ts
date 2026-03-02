import { defineStore } from 'pinia'
import { ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import type { SettingsPayload } from './types'

export const useSettingsStore = defineStore('settings', () => {
  const selectedModelId = ref('whisper:large-v3-turbo-q8')
  const selectedLanguage = ref('auto')
  const asrCloudModel = ref('whisper-1')
  const textCleanupEnabled = ref(false)
  const cleanupModelId = ref('')
  const llmModel = ref('')
  const llmMaxTokens = ref(4096)
  const hallucinationFilterEnabled = ref(true)
  const selectedInputDeviceUid = ref<string | null>(null)
  const audioDuckingEnabled = ref(false)
  const audioDuckingLevel = ref(0.8)
  const gpuMode = ref('auto')
  const hotkey = ref('right_command')
  const cancelShortcut = ref('escape')
  const recordingMode = ref('push_to_talk')
  const vadEnabled = ref(true)
  const appLocale = ref('auto')
  const theme = ref('system')

  async function fetchSettings() {
    try {
      const s = await invoke<SettingsPayload>('get_settings')
      appLocale.value = s.app_locale
      hallucinationFilterEnabled.value = s.hallucination_filter_enabled
      hotkey.value = s.hotkey
      selectedInputDeviceUid.value = s.selected_input_device_uid
      cancelShortcut.value = s.cancel_shortcut
      recordingMode.value = s.recording_mode
      selectedModelId.value = s.selected_model_id
      selectedLanguage.value = s.selected_language
      textCleanupEnabled.value = s.text_cleanup_enabled ?? false
      cleanupModelId.value = s.cleanup_model_id ?? ''
      llmModel.value = s.llm_model ?? ''
      asrCloudModel.value = s.asr_cloud_model ?? 'whisper-1'
      gpuMode.value = s.gpu_mode ?? 'auto'
      llmMaxTokens.value = s.llm_max_tokens ?? 4096
      audioDuckingEnabled.value = s.audio_ducking_enabled ?? false
      audioDuckingLevel.value = s.audio_ducking_level ?? 0.2
      vadEnabled.value = s.vad_enabled ?? true
      theme.value = s.theme ?? 'system'
    } catch (e) { console.error('fetchSettings failed:', e) }
  }

  function getSettingValue(key: string): string {
    switch (key) {
      case 'app_locale': return appLocale.value
      case 'hallucination_filter_enabled': return String(hallucinationFilterEnabled.value)
      case 'hotkey': return hotkey.value
      case 'cancel_shortcut': return cancelShortcut.value
      case 'recording_mode': return recordingMode.value
      case 'selected_input_device_uid': return selectedInputDeviceUid.value ?? ''
      case 'selected_model_id': return selectedModelId.value
      case 'selected_language': return selectedLanguage.value
      case 'text_cleanup_enabled': return String(textCleanupEnabled.value)
      case 'cleanup_model_id': return cleanupModelId.value
      case 'llm_model': return llmModel.value
      case 'asr_cloud_model': return asrCloudModel.value
      case 'gpu_mode': return gpuMode.value
      case 'llm_max_tokens': return String(llmMaxTokens.value)
      case 'audio_ducking_enabled': return String(audioDuckingEnabled.value)
      case 'audio_ducking_level': return String(audioDuckingLevel.value)
      case 'vad_enabled': return String(vadEnabled.value)
      case 'theme': return theme.value
      default: return ''
    }
  }

  function applySettingLocally(key: string, value: string) {
    switch (key) {
      case 'app_locale': appLocale.value = value; break
      case 'hallucination_filter_enabled': hallucinationFilterEnabled.value = value === 'true'; break
      case 'hotkey': hotkey.value = value; break
      case 'cancel_shortcut': cancelShortcut.value = value; break
      case 'recording_mode': recordingMode.value = value; break
      case 'selected_input_device_uid': selectedInputDeviceUid.value = value || null; break
      case 'selected_model_id': selectedModelId.value = value; break
      case 'selected_language': selectedLanguage.value = value; break
      case 'text_cleanup_enabled': textCleanupEnabled.value = value === 'true'; break
      case 'cleanup_model_id': cleanupModelId.value = value; break
      case 'llm_model': llmModel.value = value; break
      case 'asr_cloud_model': asrCloudModel.value = value; break
      case 'gpu_mode': gpuMode.value = value; break
      case 'llm_max_tokens': llmMaxTokens.value = parseInt(value, 10) || 4096; break
      case 'audio_ducking_enabled': audioDuckingEnabled.value = value === 'true'; break
      case 'audio_ducking_level': audioDuckingLevel.value = parseFloat(value) || 0.8; break
      case 'vad_enabled': vadEnabled.value = value === 'true'; break
      case 'theme': theme.value = value; break
    }
  }

  async function setSetting(key: string, value: string) {
    const prev = getSettingValue(key)
    applySettingLocally(key, value)
    try {
      await invoke('set_setting', { key, value })
    } catch (e) {
      console.error('setSetting failed, rolling back:', e)
      applySettingLocally(key, prev)
    }
  }

  async function selectModel(id: string) {
    await setSetting('selected_model_id', id)
  }

  async function selectLanguageAction(code: string) {
    await setSetting('selected_language', code)
  }

  return {
    selectedModelId, selectedLanguage, asrCloudModel,
    textCleanupEnabled, cleanupModelId, llmModel, llmMaxTokens,
    hallucinationFilterEnabled, vadEnabled, selectedInputDeviceUid,
    audioDuckingEnabled, audioDuckingLevel, gpuMode,
    hotkey, cancelShortcut, recordingMode, appLocale, theme,
    fetchSettings, setSetting, applySettingLocally, getSettingValue,
    selectModel, selectLanguageAction,
  }
})
