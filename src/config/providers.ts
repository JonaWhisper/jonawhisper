import type { Provider, ProviderPresetInfo } from '@/stores/types'

/** Check if a provider supports ASR (resolved by backend) */
export function hasAsrSupport(provider: Provider): boolean {
  return provider.supports_asr
}

/** Check if a provider supports LLM (resolved by backend) */
export function hasLlmSupport(provider: Provider): boolean {
  return provider.supports_llm
}

/** Heuristic: model ID looks like an ASR/transcription model */
function isAsrModel(id: string): boolean {
  const lower = id.toLowerCase()
  return lower.includes('whisper') || lower.includes('transcrib')
}

/** Heuristic: model ID looks like a utility model (not ASR, not LLM) */
function isUtilityModel(id: string): boolean {
  const lower = id.toLowerCase()
  return lower.includes('embed') || lower.includes('tts') || lower.includes('dall-e')
    || lower.includes('moderation') || lower.includes('text-embedding')
}

/** Get ASR models for a provider (cached > preset fallback) */
export function getAsrModels(provider: Provider, presets: ProviderPresetInfo[]): string[] {
  if (provider.cached_models.length > 0) {
    return provider.cached_models.filter(isAsrModel)
  }
  const preset = presets.find(p => p.id === provider.kind)
  return preset?.default_asr_models ?? []
}

/** Get LLM models for a provider (cached > preset fallback) */
export function getLlmModels(provider: Provider, presets: ProviderPresetInfo[]): string[] {
  if (provider.cached_models.length > 0) {
    return provider.cached_models.filter(id => !isAsrModel(id) && !isUtilityModel(id))
  }
  const preset = presets.find(p => p.id === provider.kind)
  return preset?.default_llm_models ?? []
}
