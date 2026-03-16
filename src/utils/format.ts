import i18n from '@/i18n'

function t(key: string): string {
  return (i18n.global as unknown as { t: (k: string) => string }).t(key)
}

/**
 * Format a byte count as a human-readable string.
 * Uses i18n for unit labels (FR: o/Ko/Mo/Go, EN: B/KB/MB/GB).
 * @param suffix Optional suffix appended after the unit (e.g. "/s" → "24.5 Mo/s")
 */
export function formatBytes(bytes: number, suffix = ''): string {
  if (bytes <= 0) return `0 ${t('units.bytes')}${suffix}`
  if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} ${t('units.gb')}${suffix}`
  if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(1)} ${t('units.mb')}${suffix}`
  if (bytes >= 1_000) return `${Math.round(bytes / 1_000)} ${t('units.kb')}${suffix}`
  return `${bytes} ${t('units.bytes')}${suffix}`
}

/** Format a byte count as a size (e.g. "1.6 Go"). */
export function formatSize(bytes: number): string {
  return formatBytes(bytes)
}

/** Format a bytes-per-second speed (e.g. "24.5 Mo/s"). */
export function formatSpeed(bytesPerSec: number): string {
  return formatBytes(bytesPerSec, '/s')
}

/** Format a RAM amount in human-readable form (e.g. "1.5 Go", "800 Mo"). */
export function formatRam(bytes: number): string {
  const gb = bytes / 1_000_000_000
  if (gb >= 1) return `${gb % 1 === 0 ? gb.toFixed(0) : gb.toFixed(1)} ${t('units.gb')}`
  const mb = bytes / 1_000_000
  return `${Math.round(mb)} ${t('units.mb')}`
}
