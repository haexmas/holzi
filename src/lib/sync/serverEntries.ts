// The servers of one kind as the settings and the link form list them (spec 024, FR-008): the
// built-in ones first, which can be switched off but not removed, then the ones the user added,
// which can be switched off or removed. Pure, so `scripts/check-settings.ts` tests it.

export type ServerEntry = {
  url: string
  /** In use; a switched off server stays listed. */
  enabled: boolean
  /** Built in: switched off at most, never removed. */
  isDefault: boolean
}

export function serverEntries(
  defaults: readonly string[],
  added: readonly string[],
  disabled: readonly string[],
): ServerEntry[] {
  const entries: ServerEntry[] = defaults.map((url) => ({
    url,
    enabled: !disabled.includes(url),
    isDefault: true,
  }))
  for (const url of added) {
    if (entries.some((entry) => entry.url === url)) continue
    entries.push({ url, enabled: !disabled.includes(url), isDefault: false })
  }
  return entries
}

/** `disabled` after `url` was switched on or off. */
export function withEnabled(
  disabled: readonly string[],
  url: string,
  enabled: boolean,
): string[] {
  const rest = disabled.filter((entry) => entry !== url)
  return enabled ? rest : [...rest, url]
}

/** The added servers after `url` was removed, and `disabled` without it (nothing to remember). */
export function withoutServer(
  added: readonly string[],
  disabled: readonly string[],
  url: string,
): { added: string[]; disabled: string[] } {
  return {
    added: added.filter((entry) => entry !== url),
    disabled: disabled.filter((entry) => entry !== url),
  }
}
