export type ShortcutKind = 'ModifierOnly' | 'Combo' | 'Key'

export interface ShortcutDef {
  key_codes: number[]
  modifiers: number
  kind: ShortcutKind
}

// CGEventFlags masks
const CG_MASK_CONTROL = 1 << 18
const CG_MASK_ALTERNATE = 1 << 19
const CG_MASK_COMMAND = 1 << 20
const CG_MASK_SHIFT = 1 << 17

// Rempli au demarrage depuis le backend, seul a connaitre la plateforme : les
// codes et les libelles different entierement entre CGEvent et les codes
// virtuels Windows, et une table cote Vue serait celle d'Apple partout.
let KEY_CODE_LABELS: Record<number, string> = {}
let MODIFIER_LABELS = { control: '\u2303', alternate: '\u2325', shift: '\u21e7', command: '\u2318' }
let MODIFIER_JOIN = ''

export interface KeyLabels {
  keys: Record<number, string>
  modifier_join: string
  control: string
  alternate: string
  shift: string
  command: string
}

export function applyKeyLabels(labels: KeyLabels): void {
  KEY_CODE_LABELS = labels.keys
  MODIFIER_JOIN = labels.modifier_join
  MODIFIER_LABELS = {
    control: labels.control,
    alternate: labels.alternate,
    shift: labels.shift,
    command: labels.command,
  }
}

function modifierParts(flags: number): string[] {
  const parts: string[] = []
  if (flags & CG_MASK_CONTROL) parts.push(MODIFIER_LABELS.control)
  if (flags & CG_MASK_ALTERNATE) parts.push(MODIFIER_LABELS.alternate)
  if (flags & CG_MASK_SHIFT) parts.push(MODIFIER_LABELS.shift)
  if (flags & CG_MASK_COMMAND) parts.push(MODIFIER_LABELS.command)
  return parts
}

function modifierSymbols(flags: number): string {
  return modifierParts(flags).join(MODIFIER_JOIN)
}

export function parseShortcut(s: string): ShortcutDef | null {
  if (!s) return null
  try {
    const parsed = JSON.parse(s)
    // New format: key_codes array
    if (Array.isArray(parsed.key_codes) && typeof parsed.modifiers === 'number' && parsed.kind) {
      return parsed as ShortcutDef
    }
    // Old format: key_code singular
    if (typeof parsed.key_code === 'number' && typeof parsed.modifiers === 'number' && parsed.kind) {
      const key_codes = (parsed.key_code === 0 && parsed.modifiers === 0)
        ? []
        : [parsed.key_code]
      return { key_codes, modifiers: parsed.modifiers, kind: parsed.kind }
    }
  } catch {
    // Legacy format
  }
  // Legacy string values
  const legacy: Record<string, ShortcutDef> = {
    right_command: { key_codes: [0x36], modifiers: CG_MASK_COMMAND, kind: 'ModifierOnly' },
    right_option: { key_codes: [0x3D], modifiers: CG_MASK_ALTERNATE, kind: 'ModifierOnly' },
    right_control: { key_codes: [0x3E], modifiers: CG_MASK_CONTROL, kind: 'ModifierOnly' },
    right_shift: { key_codes: [0x3C], modifiers: CG_MASK_SHIFT, kind: 'ModifierOnly' },
    escape: { key_codes: [0x35], modifiers: 0, kind: 'Key' },
    none: { key_codes: [], modifiers: 0, kind: 'Key' },
  }
  return legacy[s] ?? null
}

export function formatShortcut(s: ShortcutDef): string {
  if (isDisabled(s)) return ''
  switch (s.kind) {
    case 'ModifierOnly':
      return s.key_codes.map(kc => KEY_CODE_LABELS[kc] ?? '?').join('+')
    case 'Combo':
      return modifierSymbols(s.modifiers) + s.key_codes.map(kc => KEY_CODE_LABELS[kc] ?? '?').join('')
    case 'Key':
      return s.key_codes.map(kc => KEY_CODE_LABELS[kc] ?? '?').join('+')
  }
}

export function formatCaptureState(modifiers: number, keyCodes: number[]): string {
  let s = modifierSymbols(modifiers)
  for (const kc of keyCodes) {
    s += KEY_CODE_LABELS[kc] ?? '?'
  }
  return s || '...'
}

export function serializeShortcut(s: ShortcutDef): string {
  return JSON.stringify(s)
}

export function isDisabled(s: ShortcutDef): boolean {
  return s.key_codes.length === 0 && s.modifiers === 0
}

// Structured key cap parts for visual rendering
export interface KeyCapPart {
  symbol: string    // e.g. "⌘", "⎋", "A"
  side?: string     // e.g. "Right", "Left" (for ModifierOnly)
}

const SYMBOL_MAP: Record<number, string> = {
  0x35: '⎋', // Escape
}

export function formatShortcutParts(s: ShortcutDef): KeyCapPart[] {
  if (isDisabled(s)) return []
  switch (s.kind) {
    case 'ModifierOnly': {
      return s.key_codes.map(kc => {
        const full = KEY_CODE_LABELS[kc] ?? '?'
        const spaceIdx = full.lastIndexOf(' ')
        if (spaceIdx > 0) {
          return { symbol: full.slice(spaceIdx + 1), side: full.slice(0, spaceIdx) }
        }
        return { symbol: full }
      })
    }
    case 'Combo': {
      const parts: KeyCapPart[] = []
      for (const m of modifierParts(s.modifiers)) parts.push({ symbol: m })
      for (const kc of s.key_codes) {
        parts.push({ symbol: SYMBOL_MAP[kc] ?? KEY_CODE_LABELS[kc] ?? '?' })
      }
      return parts
    }
    case 'Key': {
      return s.key_codes.map(kc => {
        return { symbol: SYMBOL_MAP[kc] ?? KEY_CODE_LABELS[kc] ?? '?' }
      })
    }
  }
}
