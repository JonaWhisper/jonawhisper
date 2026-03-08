import { describe, it, expect } from 'vitest'
import {
  PROVIDER_PRESETS,
  PRESET_ENTRIES,
  hasAsrSupport,
  hasLlmSupport,
  getAsrModels,
  getLlmModels,
} from './providers'
import type { Provider } from '@/stores/types'

function makeProvider(overrides: Partial<Provider> = {}): Provider {
  return {
    id: 'test',
    name: 'Test',
    kind: 'OpenAI',
    url: 'https://api.openai.com',
    api_key: '',
    allow_insecure: false,
    cached_models: [],
    supports_asr: false,
    supports_llm: false,
    ...overrides,
  }
}

describe('PROVIDER_PRESETS', () => {
  it('has 9 presets', () => {
    expect(Object.keys(PROVIDER_PRESETS)).toHaveLength(9)
  })

  it('all presets have labels and HTTPS URLs', () => {
    for (const [kind, preset] of Object.entries(PROVIDER_PRESETS)) {
      expect(preset!.label, `${kind} missing label`).toBeTruthy()
      expect(preset!.url, `${kind} missing url`).toMatch(/^https:\/\//)
    }
  })

  it('includes expected providers', () => {
    const kinds = Object.keys(PROVIDER_PRESETS)
    expect(kinds).toContain('OpenAI')
    expect(kinds).toContain('Anthropic')
    expect(kinds).toContain('Groq')
    expect(kinds).toContain('Cerebras')
    expect(kinds).toContain('Gemini')
    expect(kinds).toContain('Mistral')
    expect(kinds).toContain('Fireworks')
    expect(kinds).toContain('Together')
    expect(kinds).toContain('DeepSeek')
  })

  it('OpenAI has both ASR and LLM models', () => {
    const openai = PROVIDER_PRESETS.OpenAI!
    expect(openai.asrModels!.length).toBeGreaterThan(0)
    expect(openai.llmModels!.length).toBeGreaterThan(0)
  })

  it('Anthropic has LLM models but no ASR models', () => {
    const anthropic = PROVIDER_PRESETS.Anthropic!
    expect(anthropic.asrModels).toBeUndefined()
    expect(anthropic.llmModels!.length).toBeGreaterThan(0)
  })
})

describe('PRESET_ENTRIES', () => {
  it('has same length as PROVIDER_PRESETS', () => {
    expect(PRESET_ENTRIES).toHaveLength(Object.keys(PROVIDER_PRESETS).length)
  })

  it('entries are [kind, preset] tuples', () => {
    for (const [kind, preset] of PRESET_ENTRIES) {
      expect(typeof kind).toBe('string')
      expect(preset.label).toBeTruthy()
    }
  })
})

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
    const result = getAsrModels(provider)
    expect(result).toEqual(['whisper-1', 'gpt-4o-transcribe'])
  })

  it('falls back to preset ASR models when no cached models', () => {
    const provider = makeProvider({ kind: 'OpenAI', cached_models: [] })
    const result = getAsrModels(provider)
    expect(result).toEqual(PROVIDER_PRESETS.OpenAI!.asrModels)
  })

  it('returns empty array for provider with no preset ASR', () => {
    const provider = makeProvider({ kind: 'Anthropic', cached_models: [] })
    expect(getAsrModels(provider)).toEqual([])
  })
})

describe('getLlmModels', () => {
  it('returns non-ASR non-utility cached models', () => {
    const provider = makeProvider({
      cached_models: ['gpt-4o', 'whisper-1', 'text-embedding-ada', 'dall-e-3'],
    })
    const result = getLlmModels(provider)
    expect(result).toEqual(['gpt-4o'])
  })

  it('falls back to preset LLM models when no cached models', () => {
    const provider = makeProvider({ kind: 'OpenAI', cached_models: [] })
    const result = getLlmModels(provider)
    expect(result).toEqual(PROVIDER_PRESETS.OpenAI!.llmModels)
  })

  it('filters out tts and moderation models', () => {
    const provider = makeProvider({
      cached_models: ['gpt-4o', 'tts-1', 'text-moderation-latest'],
    })
    expect(getLlmModels(provider)).toEqual(['gpt-4o'])
  })
})
