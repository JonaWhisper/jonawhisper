export type ShortcutKind = 'ModifierOnly' | 'Combo' | 'Key'

export interface ShortcutDef {
  key_code: number
  modifiers: number
  kind: ShortcutKind
}

// CGEventFlags masks
const CG_MASK_CONTROL = 1 << 18
const CG_MASK_ALTERNATE = 1 << 19
const CG_MASK_COMMAND = 1 << 20
const CG_MASK_SHIFT = 1 << 17

const KEY_CODE_LABELS: Record<number, string> = {
  // Letters
  0x00: 'A', 0x0B: 'B', 0x08: 'C', 0x02: 'D',
  0x0E: 'E', 0x03: 'F', 0x05: 'G', 0x04: 'H',
  0x22: 'I', 0x26: 'J', 0x28: 'K', 0x25: 'L',
  0x2E: 'M', 0x2D: 'N', 0x1F: 'O', 0x23: 'P',
  0x0C: 'Q', 0x0F: 'R', 0x01: 'S', 0x11: 'T',
  0x20: 'U', 0x09: 'V', 0x0D: 'W', 0x07: 'X',
  0x10: 'Y', 0x06: 'Z',
  // Numbers
  0x12: '1', 0x13: '2', 0x14: '3', 0x15: '4',
  0x17: '5', 0x16: '6', 0x1A: '7', 0x1C: '8',
  0x19: '9', 0x1D: '0',
  // F-keys
  0x7A: 'F1', 0x78: 'F2', 0x63: 'F3', 0x76: 'F4',
  0x60: 'F5', 0x61: 'F6', 0x62: 'F7', 0x64: 'F8',
  0x65: 'F9', 0x6D: 'F10', 0x67: 'F11', 0x6F: 'F12',
  0x69: 'F13', 0x6B: 'F14', 0x71: 'F15', 0x6A: 'F16',
  0x40: 'F17', 0x4F: 'F18', 0x50: 'F19', 0x5A: 'F20',
  // Special
  0x31: 'Space', 0x24: 'Return', 0x30: 'Tab',
  0x33: 'Delete', 0x75: 'Fwd Delete', 0x35: 'Escape',
  // Arrows
  0x7B: '←', 0x7C: '→', 0x7E: '↑', 0x7D: '↓',
  // Navigation
  0x73: 'Home', 0x77: 'End', 0x74: 'Page Up', 0x79: 'Page Down',
  // Punctuation
  0x1B: '-', 0x18: '=', 0x21: '[', 0x1E: ']',
  0x2A: '\\', 0x29: ';', 0x27: "'", 0x2B: ',',
  0x2F: '.', 0x2C: '/', 0x32: '`',
  // Modifiers (for ModifierOnly display)
  0x36: 'Right ⌘', 0x37: 'Left ⌘',
  0x3D: 'Right ⌥', 0x3A: 'Left ⌥',
  0x3E: 'Right ⌃', 0x3B: 'Left ⌃',
  0x3C: 'Right ⇧', 0x38: 'Left ⇧',
  0x3F: 'Fn',
}

function modifierSymbols(flags: number): string {
  let s = ''
  if (flags & CG_MASK_CONTROL) s += '⌃'
  if (flags & CG_MASK_ALTERNATE) s += '⌥'
  if (flags & CG_MASK_SHIFT) s += '⇧'
  if (flags & CG_MASK_COMMAND) s += '⌘'
  return s
}

export function parseShortcut(s: string): ShortcutDef | null {
  if (!s) return null
  try {
    const parsed = JSON.parse(s) as ShortcutDef
    if (typeof parsed.key_code === 'number' && typeof parsed.modifiers === 'number' && parsed.kind) {
      return parsed
    }
  } catch {
    // Legacy format
  }
  // Legacy string values
  const legacy: Record<string, ShortcutDef> = {
    right_command: { key_code: 0x36, modifiers: CG_MASK_COMMAND, kind: 'ModifierOnly' },
    right_option: { key_code: 0x3D, modifiers: CG_MASK_ALTERNATE, kind: 'ModifierOnly' },
    right_control: { key_code: 0x3E, modifiers: CG_MASK_CONTROL, kind: 'ModifierOnly' },
    right_shift: { key_code: 0x3C, modifiers: CG_MASK_SHIFT, kind: 'ModifierOnly' },
    escape: { key_code: 0x35, modifiers: 0, kind: 'Key' },
    none: { key_code: 0, modifiers: 0, kind: 'Key' },
  }
  return legacy[s] ?? null
}

export function formatShortcut(s: ShortcutDef): string {
  if (isDisabled(s)) return ''
  switch (s.kind) {
    case 'ModifierOnly':
      return KEY_CODE_LABELS[s.key_code] ?? '⌘'
    case 'Combo':
      return modifierSymbols(s.modifiers) + (KEY_CODE_LABELS[s.key_code] ?? '?')
    case 'Key':
      return KEY_CODE_LABELS[s.key_code] ?? '?'
  }
}

export function formatCaptureState(modifiers: number, keyCode: number | null): string {
  let s = modifierSymbols(modifiers)
  if (keyCode != null && keyCode > 0) {
    s += KEY_CODE_LABELS[keyCode] ?? '?'
  }
  return s || '...'
}

export function serializeShortcut(s: ShortcutDef): string {
  return JSON.stringify(s)
}

export function isDisabled(s: ShortcutDef): boolean {
  return s.key_code === 0 && s.modifiers === 0
}
