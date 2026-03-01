import type { ProviderKind } from '@/stores/app'

export interface ProviderPreset {
  label: string
  url: string
  asrModels?: string[]
  llmModels?: string[]
}

export const PROVIDER_PRESETS: Partial<Record<ProviderKind, ProviderPreset>> = {
  OpenAI: {
    label: 'OpenAI',
    url: 'https://api.openai.com',
    asrModels: ['whisper-1', 'gpt-4o-transcribe', 'gpt-4o-mini-transcribe'],
    llmModels: ['gpt-4o-mini', 'gpt-4o'],
  },
  Anthropic: {
    label: 'Anthropic',
    url: 'https://api.anthropic.com',
    llmModels: ['claude-haiku-4-5-20251001', 'claude-sonnet-4-5-20250514', 'claude-opus-4-6-20250626'],
  },
  Groq: {
    label: 'Groq',
    url: 'https://api.groq.com/openai/v1',
    asrModels: ['whisper-large-v3-turbo', 'whisper-large-v3'],
    llmModels: ['llama-3.1-8b-instant'],
  },
  Cerebras: {
    label: 'Cerebras',
    url: 'https://api.cerebras.ai/v1',
    llmModels: ['llama3.1-8b'],
  },
  Gemini: {
    label: 'Google Gemini',
    url: 'https://generativelanguage.googleapis.com/v1beta/openai',
    llmModels: ['gemini-2.5-flash-lite'],
  },
  Mistral: {
    label: 'Mistral',
    url: 'https://api.mistral.ai/v1',
    llmModels: ['ministral-3b-latest'],
  },
  Fireworks: {
    label: 'Fireworks AI',
    url: 'https://api.fireworks.ai/inference/v1',
    asrModels: ['whisper-v3-turbo', 'whisper-v3'],
  },
  Together: {
    label: 'Together AI',
    url: 'https://api.together.xyz/v1',
    asrModels: ['openai/whisper-large-v3'],
    llmModels: ['meta-llama/Llama-3.2-3B'],
  },
  DeepSeek: {
    label: 'DeepSeek',
    url: 'https://api.deepseek.com/v1',
    llmModels: ['deepseek-v3.2'],
  },
}

/** Ordered list of preset entries for UI dropdowns (Custom excluded — handled separately) */
export const PRESET_ENTRIES = Object.entries(PROVIDER_PRESETS) as [ProviderKind, ProviderPreset][]

/** Check if a provider kind has ASR models available */
export function hasAsrSupport(kind: ProviderKind): boolean {
  if (kind === 'Custom') return true
  return (PROVIDER_PRESETS[kind]?.asrModels?.length ?? 0) > 0
}
