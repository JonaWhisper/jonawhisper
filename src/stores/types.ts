export interface AudioDevice {
  id: number
  name: string
  uid: string
  transport_type: string
  is_default: boolean
}

export interface EngineInfo {
  id: string
  name: string
  description: string
  category: 'asr' | 'llm' | 'punctuation' | 'correction' | 'spellcheck' | 'languagemodel'
  available: boolean
  supported_language_codes: string[]
}

export interface ASRModel {
  id: string
  engine_id: string
  label: string
  filename: string
  url: string
  size: number
  storage_dir: string
  download_type: { type: string }
  download_marker: string | null
  is_downloaded?: boolean
  recommended?: boolean
  partial_progress: number | null
  wer: number | null
  rtf: number | null
  params: number | null
  ram: number | null
  lang_codes: string[] | null
  quantization: string | null
}

export interface Language {
  code: string
  label: string
}

export interface HistoryEntry {
  text: string
  timestamp: number
  model_id: string
  language: string
  cleanup_model_id: string
  hallucination_filter: boolean
  vad_trimmed: boolean
  punctuation_model_id: string
  spellcheck: boolean
  disfluency_removal: boolean
  itn: boolean
}

export type ProviderKind = 'OpenAI' | 'Anthropic' | 'Custom' | 'Groq' | 'Cerebras' | 'Gemini' | 'Mistral' | 'Fireworks' | 'Together' | 'DeepSeek'

export interface Provider {
  id: string
  name: string
  kind: ProviderKind
  url: string
  api_key: string
  allow_insecure: boolean
  cached_models: string[]
  supports_asr: boolean
  supports_llm: boolean
}

export interface CleanupModel {
  id: string
  label: string
  group: 'punctuation' | 'llm' | 'cloud' | 'correction'
  params: number | null
  ram: number | null
  lang_codes: string[] | null
  quantization: string | null
  recommended: boolean
}

export interface AsrModelOption {
  id: string
  label: string
  group: 'local' | 'cloud'
  params: number | null
  ram: number | null
  wer: number | null
  rtf: number | null
  lang_codes: string[] | null
  quantization: string | null
  recommended: boolean
}

export interface PermissionReport {
  microphone: string
  accessibility: string
  input_monitoring: string
}

// Tauri event payload types
export interface RecordingStoppedPayload {
  queue_count?: number
}

export interface TranscriptionStartedPayload {
  queue_count?: number
}

export interface TranscriptionCompletePayload {
  text?: string
  cleanup_model_id?: string
  hallucination_filter?: boolean
  vad_trimmed?: boolean
  punctuation_model_id?: string
  spellcheck?: boolean
  disfluency_removal?: boolean
  itn?: boolean
}

export interface DownloadProgressPayload {
  model_id?: string
  progress?: number
  downloaded?: number
  total_size?: number
  speed?: number
}

export interface AppStatePayload {
  is_recording: boolean
  is_transcribing: boolean
  queue_count: number
  active_downloads: Record<string, number>
}

// --- Helpers ---

/** True if the model is usable (downloaded, system-provided, or remote API) */
export function isModelAvailable(model: ASRModel): boolean {
  const dt = model.download_type.type
  if (dt === 'RemoteAPI' || dt === 'System') return true
  return !!model.is_downloaded
}

/** Parse a "cloud:<providerId>" composite ID. Returns the provider ID or null. */
export function parseCloudId(id: string): string | null {
  return id.startsWith('cloud:') ? id.slice('cloud:'.length) : null
}

// --- Tauri payloads ---

export interface SettingsPayload {
  app_locale: string
  hallucination_filter_enabled: boolean
  hotkey: string
  cancel_shortcut: string
  recording_mode: string
  selected_input_device_uid: string | null
  selected_model_id: string
  selected_language: string
  text_cleanup_enabled: boolean
  cleanup_model_id: string
  llm_provider_id: string
  llm_model: string
  asr_cloud_model: string
  gpu_mode: string
  llm_max_tokens: number
  audio_ducking_enabled: boolean
  audio_ducking_level: number
  vad_enabled: boolean
  punctuation_model_id: string
  disfluency_removal_enabled: boolean
  itn_enabled: boolean
  spellcheck_enabled: boolean
  theme: string
}
