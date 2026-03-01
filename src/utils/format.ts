/**
 * Format a byte count as a human-readable string.
 * Automatically picks the best unit (o, Ko, Mo, Go).
 * @param suffix Optional suffix appended after the unit (e.g. "/s" → "24.5 Mo/s")
 */
export function formatBytes(bytes: number, suffix = ''): string {
  if (bytes <= 0) return `0 o${suffix}`
  if (bytes >= 1_000_000_000) return `${(bytes / 1_000_000_000).toFixed(1)} Go${suffix}`
  if (bytes >= 1_000_000) return `${(bytes / 1_000_000).toFixed(1)} Mo${suffix}`
  if (bytes >= 1_000) return `${Math.round(bytes / 1_000)} Ko${suffix}`
  return `${bytes} o${suffix}`
}

/** Format a byte count as a size (e.g. "1.6 Go"). */
export function formatSize(bytes: number): string {
  return formatBytes(bytes)
}

/** Format a bytes-per-second speed (e.g. "24.5 Mo/s"). */
export function formatSpeed(bytesPerSec: number): string {
  return formatBytes(bytesPerSec, '/s')
}

/** Format a RAM amount in human-readable form (e.g. "1.5 GB", "800 MB"). */
export function formatRam(bytes: number): string {
  const gb = bytes / 1_000_000_000
  if (gb >= 1) return `${gb % 1 === 0 ? gb.toFixed(0) : gb.toFixed(1)} GB`
  const mb = bytes / 1_000_000
  return `${Math.round(mb)} MB`
}
