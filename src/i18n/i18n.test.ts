import { describe, it, expect } from 'vitest'
import en from './en.json'
import fr from './fr.json'

function getKeys(obj: Record<string, unknown>): string[] {
  return Object.keys(obj).filter(k => !k.startsWith('_'))
}

describe('i18n completeness', () => {
  const enKeys = getKeys(en)
  const frKeys = getKeys(fr)

  it('en.json has translation keys', () => {
    expect(enKeys.length).toBeGreaterThan(0)
  })

  it('fr.json has translation keys', () => {
    expect(frKeys.length).toBeGreaterThan(0)
  })

  it('en and fr have the same number of keys', () => {
    expect(enKeys.length).toBe(frKeys.length)
  })

  it('every en key exists in fr', () => {
    const frKeySet = new Set(frKeys)
    const missingInFr = enKeys.filter(k => !frKeySet.has(k))
    expect(missingInFr, `Keys in en.json missing from fr.json: ${missingInFr.join(', ')}`).toEqual([])
  })

  it('every fr key exists in en', () => {
    const enKeySet = new Set(enKeys)
    const missingInEn = frKeys.filter(k => !enKeySet.has(k))
    expect(missingInEn, `Keys in fr.json missing from en.json: ${missingInEn.join(', ')}`).toEqual([])
  })

  it('no translation value is empty', () => {
    for (const key of enKeys) {
      expect((en as unknown as Record<string, string>)[key], `en.${key} is empty`).toBeTruthy()
    }
    for (const key of frKeys) {
      expect((fr as unknown as Record<string, string>)[key], `fr.${key} is empty`).toBeTruthy()
    }
  })

  it('both files have matching _version', () => {
    expect((en as Record<string, unknown>)['_version']).toBe((fr as Record<string, unknown>)['_version'])
  })
})
