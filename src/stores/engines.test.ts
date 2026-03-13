import { describe, it, expect, beforeEach, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'

// Track invoke calls and event listeners
const mockInvoke = vi.fn()
vi.mock('@tauri-apps/api/core', () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}))
const listeners: Record<string, (event: unknown) => void> = {}
vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn((event: string, handler: (event: unknown) => void) => {
    listeners[event] = handler
    return () => { delete listeners[event] }
  }),
}))

import { useEnginesStore } from './engines'
import { useSettingsStore } from './settings'
import type { ASRModel, EngineInfo, Provider } from './types'

function makeEngine(overrides: Partial<EngineInfo> = {}): EngineInfo {
  return {
    id: 'test-engine',
    name: 'Test Engine',
    description: 'A test engine',
    category: 'asr',
    available: true,
    supported_language_codes: ['en', 'fr'],
    ...overrides,
  }
}

function makeModel(overrides: Partial<ASRModel> = {}): ASRModel {
  return {
    id: 'test-model',
    engine_id: 'test-engine',
    label: 'Test Model',
    filename: 'test.onnx',
    url: 'https://example.com/test.onnx',
    size: 100_000_000,
    storage_dir: '/tmp/models',
    download_type: { type: 'SingleFile' },
    download_marker: null,
    is_downloaded: true,
    recommended: false,
    partial_progress: null,
    wer: 5.0,
    rtf: 0.1,
    params: 60_000_000,
    ram: 200_000_000,
    lang_codes: ['en', 'fr'],
    quantization: 'q8',
    ...overrides,
  }
}

function makeProvider(overrides: Partial<Provider> = {}): Provider {
  return {
    id: 'test-provider',
    name: 'Test Provider',
    kind: 'OpenAI',
    url: 'https://api.openai.com',
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

describe('engines store computed properties', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockInvoke.mockReset()
    Object.keys(listeners).forEach(k => delete listeners[k])
  })

  it('downloadedModels filters to available models', () => {
    const store = useEnginesStore()
    store.models = [
      makeModel({ id: 'a', is_downloaded: true }),
      makeModel({ id: 'b', is_downloaded: false }),
      makeModel({ id: 'c', download_type: { type: 'RemoteAPI' }, is_downloaded: false }),
    ]
    expect(store.downloadedModels.map(m => m.id)).toEqual(['a', 'c'])
  })

  it('asrEngines filters by category', () => {
    const store = useEnginesStore()
    store.engines = [
      makeEngine({ id: 'whisper', category: 'asr' }),
      makeEngine({ id: 'llama', category: 'llm' }),
      makeEngine({ id: 'bert', category: 'punctuation' }),
    ]
    expect(store.asrEngines.map(e => e.id)).toEqual(['whisper'])
  })

  it('llmEngines filters by category', () => {
    const store = useEnginesStore()
    store.engines = [
      makeEngine({ id: 'whisper', category: 'asr' }),
      makeEngine({ id: 'llama', category: 'llm' }),
    ]
    expect(store.llmEngines.map(e => e.id)).toEqual(['llama'])
  })

  it('punctuationEngines filters by category', () => {
    const store = useEnginesStore()
    store.engines = [
      makeEngine({ id: 'bert', category: 'punctuation' }),
      makeEngine({ id: 'whisper', category: 'asr' }),
    ]
    expect(store.punctuationEngines.map(e => e.id)).toEqual(['bert'])
  })

  it('correctionEngines filters by category', () => {
    const store = useEnginesStore()
    store.engines = [
      makeEngine({ id: 't5', category: 'correction' }),
      makeEngine({ id: 'whisper', category: 'asr' }),
    ]
    expect(store.correctionEngines.map(e => e.id)).toEqual(['t5'])
  })

  it('punctuationModels maps downloaded punctuation models', () => {
    const store = useEnginesStore()
    store.engines = [makeEngine({ id: 'bert', category: 'punctuation' })]
    store.models = [
      makeModel({ id: 'bert-base', engine_id: 'bert', is_downloaded: true, params: 110_000_000 }),
      makeModel({ id: 'bert-large', engine_id: 'bert', is_downloaded: false }),
    ]
    expect(store.punctuationModels).toHaveLength(1)
    expect(store.punctuationModels[0]!.id).toBe('bert-base')
    expect(store.punctuationModels[0]!.group).toBe('punctuation')
  })

  it('cleanupModels includes correction, llm, and cloud providers', () => {
    const store = useEnginesStore()
    store.engines = [
      makeEngine({ id: 't5', category: 'correction' }),
      makeEngine({ id: 'llama', category: 'llm' }),
    ]
    store.models = [
      makeModel({ id: 'gec-t5', engine_id: 't5', is_downloaded: true }),
      makeModel({ id: 'llama-7b', engine_id: 'llama', is_downloaded: true }),
    ]
    store.providers = [
      makeProvider({ id: 'openai', supports_llm: true }),
      makeProvider({ id: 'groq', supports_llm: false }),
    ]

    const cleanup = store.cleanupModels
    expect(cleanup.map(m => m.id)).toEqual(['gec-t5', 'llama-7b', 'cloud:openai'])
    expect(cleanup[0]!.group).toBe('correction')
    expect(cleanup[1]!.group).toBe('llm')
    expect(cleanup[2]!.group).toBe('cloud')
  })

  it('asrModels includes local downloaded + cloud ASR providers', () => {
    const store = useEnginesStore()
    store.engines = [makeEngine({ id: 'whisper', category: 'asr' })]
    store.models = [
      makeModel({ id: 'whisper-large', engine_id: 'whisper', is_downloaded: true }),
      makeModel({ id: 'whisper-small', engine_id: 'whisper', is_downloaded: false }),
    ]
    store.providers = [
      makeProvider({ id: 'openai', supports_asr: true }),
      makeProvider({ id: 'anthropic', supports_asr: false }),
    ]

    const asr = store.asrModels
    expect(asr.map(m => m.id)).toEqual(['whisper-large', 'cloud:openai'])
    expect(asr[0]!.group).toBe('local')
    expect(asr[1]!.group).toBe('cloud')
  })

  it('isCloudAsr detects cloud: prefix in selectedModelId', () => {
    const settings = useSettingsStore()
    const store = useEnginesStore()

    settings.selectedModelId = 'whisper:large-v3'
    expect(store.isCloudAsr).toBe(false)

    settings.selectedModelId = 'cloud:openai'
    expect(store.isCloudAsr).toBe(true)
  })

  it('isCloudLlm detects cloud: prefix in cleanupModelId', () => {
    const settings = useSettingsStore()
    const store = useEnginesStore()

    settings.cleanupModelId = 'llama:7b'
    expect(store.isCloudLlm).toBe(false)

    settings.cleanupModelId = 'cloud:anthropic'
    expect(store.isCloudLlm).toBe(true)
  })

  it('isLocalLlm detects llama: prefix', () => {
    const settings = useSettingsStore()
    const store = useEnginesStore()

    settings.cleanupModelId = 'llama:7b'
    expect(store.isLocalLlm).toBe(true)

    settings.cleanupModelId = 'cloud:openai'
    expect(store.isLocalLlm).toBe(false)
  })

  it('hasSpellcheckDict detects downloaded spellcheck models', () => {
    const store = useEnginesStore()
    store.engines = [makeEngine({ id: 'symspell', category: 'spellcheck' })]

    store.models = [makeModel({ id: 'dict-fr', engine_id: 'symspell', is_downloaded: false })]
    expect(store.hasSpellcheckDict).toBe(false)

    store.models = [makeModel({ id: 'dict-fr', engine_id: 'symspell', is_downloaded: true })]
    expect(store.hasSpellcheckDict).toBe(true)
  })

  it('hasUpdate checks updatableModelIds set', () => {
    const store = useEnginesStore()
    store.updatableModelIds = new Set(['model-a', 'model-b'])
    expect(store.hasUpdate('model-a')).toBe(true)
    expect(store.hasUpdate('model-c')).toBe(false)
  })
})

describe('engines store actions', () => {
  beforeEach(() => {
    setActivePinia(createPinia())
    mockInvoke.mockReset()
    Object.keys(listeners).forEach(k => delete listeners[k])
  })

  it('fetchEngines populates engines from invoke', async () => {
    const store = useEnginesStore()
    const data = [makeEngine({ id: 'whisper' }), makeEngine({ id: 'canary' })]
    mockInvoke.mockResolvedValueOnce(data)

    await store.fetchEngines()

    expect(store.engines).toEqual(data)
    expect(mockInvoke).toHaveBeenCalledWith('get_engines')
  })

  it('fetchModels populates models from invoke', async () => {
    const store = useEnginesStore()
    const settings = useSettingsStore()
    settings.selectedModelId = ''
    store.engines = [makeEngine({ id: 'test-engine', category: 'asr' })]
    const data = [makeModel({ id: 'whisper:tiny', engine_id: 'test-engine', is_downloaded: true })]
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_models') return data
      return undefined
    })

    await store.fetchModels()

    expect(store.models).toEqual(data)
  })

  it('validateSelections resets selectedModelId when model is deleted', async () => {
    const store = useEnginesStore()
    const settings = useSettingsStore()
    settings.selectedModelId = 'whisper:deleted-model'
    store.engines = [makeEngine({ id: 'whisper', category: 'asr' })]
    store.models = [makeModel({ id: 'whisper:tiny', engine_id: 'whisper', is_downloaded: true })]
    mockInvoke.mockResolvedValue(undefined)

    // fetchModels triggers validateSelections
    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_models') return store.models
      return undefined
    })
    await store.fetchModels()

    expect(settings.selectedModelId).toBe('whisper:tiny')
  })

  it('validateSelections resets cleanupModelId when model is deleted', async () => {
    const store = useEnginesStore()
    const settings = useSettingsStore()
    settings.cleanupModelId = 'correction:deleted'
    store.engines = [makeEngine({ id: 'whisper', category: 'asr' })]
    store.models = [makeModel({ id: 'whisper:tiny', engine_id: 'whisper', is_downloaded: true })]
    mockInvoke.mockResolvedValue(undefined)

    mockInvoke.mockImplementation(async (cmd: string) => {
      if (cmd === 'get_models') return store.models
      return undefined
    })
    await store.fetchModels()

    expect(settings.cleanupModelId).toBe('')
  })

  it('model-updates-available event updates updatableModelIds', () => {
    const store = useEnginesStore()
    store.setupListeners()

    listeners['model-updates-available']!({ payload: ['model-a', 'model-b'] })

    expect(store.updatableModelIds).toEqual(new Set(['model-a', 'model-b']))
  })
})
