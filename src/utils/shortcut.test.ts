import { describe, it, expect } from 'vitest'
import {
  parseShortcut,
  formatShortcut,
  formatShortcutParts,
  formatCaptureState,
  serializeShortcut,
  isDisabled,
  type ShortcutDef,
} from './shortcut'

describe('parseShortcut', () => {
  it('returns null for empty string', () => {
    expect(parseShortcut('')).toBeNull()
  })

  it('parses new format with key_codes array', () => {
    const input = JSON.stringify({ key_codes: [0x36], modifiers: 1 << 20, kind: 'ModifierOnly' })
    const result = parseShortcut(input)
    expect(result).toEqual({ key_codes: [0x36], modifiers: 1 << 20, kind: 'ModifierOnly' })
  })

  it('parses old format with singular key_code', () => {
    const input = JSON.stringify({ key_code: 0x36, modifiers: 1 << 20, kind: 'ModifierOnly' })
    const result = parseShortcut(input)
    expect(result).toEqual({ key_codes: [0x36], modifiers: 1 << 20, kind: 'ModifierOnly' })
  })

  it('converts old format key_code=0 modifiers=0 to empty key_codes', () => {
    const input = JSON.stringify({ key_code: 0, modifiers: 0, kind: 'Key' })
    const result = parseShortcut(input)
    expect(result).toEqual({ key_codes: [], modifiers: 0, kind: 'Key' })
  })

  it('parses legacy string "right_command"', () => {
    const result = parseShortcut('right_command')
    expect(result).toEqual({ key_codes: [0x36], modifiers: 1 << 20, kind: 'ModifierOnly' })
  })

  it('parses legacy string "escape"', () => {
    const result = parseShortcut('escape')
    expect(result).toEqual({ key_codes: [0x35], modifiers: 0, kind: 'Key' })
  })

  it('parses legacy string "none"', () => {
    const result = parseShortcut('none')
    expect(result).toEqual({ key_codes: [], modifiers: 0, kind: 'Key' })
  })

  it('returns null for unknown legacy string', () => {
    expect(parseShortcut('unknown_key')).toBeNull()
  })

  it('returns null for invalid JSON without kind', () => {
    expect(parseShortcut('{"foo": 1}')).toBeNull()
  })
})

describe('isDisabled', () => {
  it('returns true for empty key_codes and zero modifiers', () => {
    expect(isDisabled({ key_codes: [], modifiers: 0, kind: 'Key' })).toBe(true)
  })

  it('returns false when key_codes has entries', () => {
    expect(isDisabled({ key_codes: [0x36], modifiers: 0, kind: 'Key' })).toBe(false)
  })

  it('returns false when modifiers is nonzero', () => {
    expect(isDisabled({ key_codes: [], modifiers: 1 << 20, kind: 'ModifierOnly' })).toBe(false)
  })
})

describe('formatShortcut', () => {
  it('returns empty string for disabled shortcut', () => {
    expect(formatShortcut({ key_codes: [], modifiers: 0, kind: 'Key' })).toBe('')
  })

  it('formats ModifierOnly shortcut', () => {
    const s: ShortcutDef = { key_codes: [0x36], modifiers: 1 << 20, kind: 'ModifierOnly' }
    expect(formatShortcut(s)).toBe('Right \u2318')
  })

  it('formats Combo shortcut (Cmd+A)', () => {
    const s: ShortcutDef = { key_codes: [0x00], modifiers: 1 << 20, kind: 'Combo' }
    expect(formatShortcut(s)).toBe('\u2318A')
  })

  it('formats Combo shortcut with multiple modifiers (Ctrl+Shift+A)', () => {
    const s: ShortcutDef = { key_codes: [0x00], modifiers: (1 << 18) | (1 << 17), kind: 'Combo' }
    expect(formatShortcut(s)).toBe('\u2303\u21e7A')
  })

  it('formats Key shortcut (Escape)', () => {
    const s: ShortcutDef = { key_codes: [0x35], modifiers: 0, kind: 'Key' }
    expect(formatShortcut(s)).toBe('Escape')
  })

  it('formats multi-key ModifierOnly (Cmd+Shift)', () => {
    const s: ShortcutDef = { key_codes: [0x36, 0x3C], modifiers: (1 << 20) | (1 << 17), kind: 'ModifierOnly' }
    expect(formatShortcut(s)).toBe('Right \u2318+Right \u21e7')
  })
})

describe('formatShortcutParts', () => {
  it('returns empty array for disabled shortcut', () => {
    expect(formatShortcutParts({ key_codes: [], modifiers: 0, kind: 'Key' })).toEqual([])
  })

  it('splits ModifierOnly into symbol and side', () => {
    const parts = formatShortcutParts({ key_codes: [0x36], modifiers: 1 << 20, kind: 'ModifierOnly' })
    expect(parts).toEqual([{ symbol: '\u2318', side: 'Right' }])
  })

  it('returns modifier symbols + key for Combo', () => {
    const parts = formatShortcutParts({ key_codes: [0x00], modifiers: 1 << 20, kind: 'Combo' })
    expect(parts).toEqual([{ symbol: '\u2318' }, { symbol: 'A' }])
  })

  it('uses SYMBOL_MAP for Escape in Key kind', () => {
    const parts = formatShortcutParts({ key_codes: [0x35], modifiers: 0, kind: 'Key' })
    expect(parts).toEqual([{ symbol: '\u238b' }])
  })

  it('handles Left modifier', () => {
    const parts = formatShortcutParts({ key_codes: [0x37], modifiers: 1 << 20, kind: 'ModifierOnly' })
    expect(parts).toEqual([{ symbol: '\u2318', side: 'Left' }])
  })
})

describe('formatCaptureState', () => {
  it('returns "..." when no modifiers and no keys', () => {
    expect(formatCaptureState(0, [])).toBe('...')
  })

  it('returns modifier symbols when only modifiers pressed', () => {
    expect(formatCaptureState(1 << 20, [])).toBe('\u2318')
  })

  it('returns modifier symbols + key labels', () => {
    expect(formatCaptureState(1 << 20, [0x00])).toBe('\u2318A')
  })

  it('returns "?" for unknown key code', () => {
    expect(formatCaptureState(0, [0xFF])).toBe('?')
  })
})

describe('serializeShortcut', () => {
  it('round-trips through parseShortcut', () => {
    const original: ShortcutDef = { key_codes: [0x36], modifiers: 1 << 20, kind: 'ModifierOnly' }
    const serialized = serializeShortcut(original)
    const parsed = parseShortcut(serialized)
    expect(parsed).toEqual(original)
  })
})
