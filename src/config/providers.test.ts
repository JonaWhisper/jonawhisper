import { describe, it, expect } from 'vitest'
import {
  hasAsrSupport,
  hasLlmSupport,
  getAsrModels,
  getLlmModels,
} from './providers'
import type { Provider, ProviderPresetInfo } from '@/stores/types'

function makeProvider(overrides: Partial<Provider> = {}): Provider {
  return {
    id: 'test',
    name: 'Test',
    kind: 'openai',
    url: 'https://api.openai.com/v1',
    api_key: '',
    allow_insecure: false,
    cached_models: [],
    supports_asr: false,
    supports_llm: false,
    enabled: true,
    source: null,
    extra: {},
    ...overrides,
  }
}

const TEST_PRESETS: ProviderPresetInfo[] = [
  {
    id: 'openai', display_name: 'OpenAI', base_url: 'https://api.openai.com/v1',
    supports_asr: true, supports_llm: true, gradient: '',
    default_asr_models: ['whisper-1', 'gpt-4o-transcribe', 'gpt-4o-mini-transcribe'],
    default_llm_models: ['gpt-4o-mini', 'gpt-4o'],
    extra_fields: [], hidden_fields: [],
  },
  {
    id: 'anthropic', display_name: 'Anthropic', base_url: 'https://api.anthropic.com/v1',
    supports_asr: false, supports_llm: true, gradient: '',
    default_asr_models: [],
    default_llm_models: ['claude-haiku-4-5-20251001'],
    extra_fields: [], hidden_fields: [],
  },
]

describe('hasAsrSupport / hasLlmSupport', () => {
  it('returns supports_asr from provider', () => {
    expect(hasAsrSupport(makeProvider({ supports_asr: true }))).toBe(true)
    expect(hasAsrSupport(makeProvider({ supports_asr: false }))).toBe(false)
  })

  it('returns supports_llm from provider', () => {
    expect(hasLlmSupport(makeProvider({ supports_llm: true }))).toBe(true)
    expect(hasLlmSupport(makeProvider({ supports_llm: false }))).toBe(false)
  })
})

describe('getAsrModels', () => {
  it('returns filtered cached models when available', () => {
    const provider = makeProvider({
      cached_models: ['whisper-1', 'gpt-4o', 'gpt-4o-transcribe', 'text-embedding-ada'],
    })
    const result = getAsrModels(provider, TEST_PRESETS)
    expect(result).toEqual(['whisper-1', 'gpt-4o-transcribe'])
  })

  it('falls back to preset ASR models when no cached models', () => {
    const provider = makeProvider({ kind: 'openai', cached_models: [] })
    const result = getAsrModels(provider, TEST_PRESETS)
    expect(result).toEqual(['whisper-1', 'gpt-4o-transcribe', 'gpt-4o-mini-transcribe'])
  })

  it('returns empty array for provider with no preset ASR', () => {
    const provider = makeProvider({ kind: 'anthropic', cached_models: [] })
    expect(getAsrModels(provider, TEST_PRESETS)).toEqual([])
  })
})

describe('getLlmModels', () => {
  it('returns non-ASR non-utility cached models', () => {
    const provider = makeProvider({
      cached_models: ['gpt-4o', 'whisper-1', 'text-embedding-ada', 'dall-e-3'],
    })
    const result = getLlmModels(provider, TEST_PRESETS)
    expect(result).toEqual(['gpt-4o'])
  })

  it('falls back to preset LLM models when no cached models', () => {
    const provider = makeProvider({ kind: 'openai', cached_models: [] })
    const result = getLlmModels(provider, TEST_PRESETS)
    expect(result).toEqual(['gpt-4o-mini', 'gpt-4o'])
  })

  it('filters out tts and moderation models', () => {
    const provider = makeProvider({
      cached_models: ['gpt-4o', 'tts-1', 'text-moderation-latest'],
    })
    expect(getLlmModels(provider, TEST_PRESETS)).toEqual(['gpt-4o'])
  })
})
