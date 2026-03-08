import { describe, it, expect, vi } from 'vitest'

// Mock i18n module before importing format utils
vi.mock('@/i18n', () => {
  const translations: Record<string, string> = {
    'units.bytes': 'o',
    'units.kb': 'Ko',
    'units.mb': 'Mo',
    'units.gb': 'Go',
  }
  return {
    default: {
      global: {
        t: (key: string) => translations[key] ?? key,
      },
    },
  }
})

import { formatBytes, formatSize, formatSpeed, formatRam } from './format'

describe('formatBytes', () => {
  it('formats zero bytes', () => {
    expect(formatBytes(0)).toBe('0 o')
  })

  it('formats negative as zero', () => {
    expect(formatBytes(-100)).toBe('0 o')
  })

  it('formats small byte values', () => {
    expect(formatBytes(512)).toBe('512 o')
  })

  it('formats kilobytes', () => {
    expect(formatBytes(1_500)).toBe('2 Ko')
  })

  it('formats megabytes', () => {
    expect(formatBytes(24_500_000)).toBe('24.5 Mo')
  })

  it('formats gigabytes', () => {
    expect(formatBytes(1_600_000_000)).toBe('1.6 Go')
  })

  it('appends suffix', () => {
    expect(formatBytes(24_500_000, '/s')).toBe('24.5 Mo/s')
  })

  it('appends suffix to zero', () => {
    expect(formatBytes(0, '/s')).toBe('0 o/s')
  })
})

describe('formatSize', () => {
  it('delegates to formatBytes without suffix', () => {
    expect(formatSize(1_600_000_000)).toBe('1.6 Go')
  })
})

describe('formatSpeed', () => {
  it('formats with /s suffix', () => {
    expect(formatSpeed(24_500_000)).toBe('24.5 Mo/s')
  })

  it('formats zero speed', () => {
    expect(formatSpeed(0)).toBe('0 o/s')
  })
})

describe('formatRam', () => {
  it('formats gigabytes with one decimal', () => {
    expect(formatRam(1_500_000_000)).toBe('1.5 GB')
  })

  it('formats exact gigabytes without decimal', () => {
    expect(formatRam(2_000_000_000)).toBe('2 GB')
  })

  it('formats megabytes', () => {
    expect(formatRam(800_000_000)).toBe('800 MB')
  })

  it('formats small megabytes', () => {
    expect(formatRam(50_000_000)).toBe('50 MB')
  })
})
