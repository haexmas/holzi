// Formatting shared by the model views (spec 023-settings-app, research R10). Pure.

/** A byte count as KB, MB or GB for download and size labels. */
export function humanBytes(bytes: number): string {
  const kb = 1024
  const mb = kb * 1024
  const gb = mb * 1024
  if (bytes >= gb) return `${(bytes / gb).toFixed(1)} GB`
  if (bytes >= mb) return `${(bytes / mb).toFixed(0)} MB`
  return `${(bytes / kb).toFixed(0)} KB`
}

/** Progress in whole percent, `null` while the total size is unknown. */
export function progressPercent(
  bytesDownloaded: number,
  bytesTotal: number | null,
): number | null {
  if (bytesTotal === null || bytesTotal <= 0) return null
  return Math.min(
    100,
    Math.max(0, Math.round((bytesDownloaded / bytesTotal) * 100)),
  )
}
