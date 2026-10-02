// Small pure helpers of the password manager window (spec 034-password-manager): how a header is
// named, when an entry counts as expired.

/** The text a list shows for an entry: its title, or `null` when there is none and the window
 * shows the placeholder `passwords.untitled` instead. The placeholder is a text of the window,
 * never a stored value (FR-001). */
export function displayTitle(title: string | null | undefined): string | null {
  const trimmed = title?.trim()
  return trimmed ? trimmed : null
}

/** `YYYY-MM-DD` of a date in the local time zone. */
export function localDay(date: Date): string {
  const month = String(date.getMonth() + 1).padStart(2, '0')
  const day = String(date.getDate()).padStart(2, '0')
  return `${date.getFullYear()}-${month}-${day}`
}

/** True when the expiry day (`YYYY-MM-DD`) lies before `today` (FR-008). An entry expires at the
 * end of its day; an unreadable value does not mark the entry. */
export function isExpired(
  expiresAt: string | null | undefined,
  today: string,
): boolean {
  if (!expiresAt || !/^\d{4}-\d{2}-\d{2}$/.test(expiresAt)) return false
  return expiresAt < today
}
