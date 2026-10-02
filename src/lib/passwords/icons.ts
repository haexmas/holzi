// Icons and colors of entries and folders (spec 034-password-manager): a fixed list the pickers
// offer, as literal `lucide:` names so the icon scan of the Nuxt build bundles them. An entry may
// also hold another stored name (an import maps KeePass icons to names of this list, research R12);
// `binary:<hash>` is a picture of the import, shown through the icon cache.

/** The icons of the picker, in the order it shows them. */
export const ENTRY_ICONS = [
  'lucide:key-round',
  'lucide:mail',
  'lucide:globe',
  'lucide:landmark',
  'lucide:credit-card',
  'lucide:shield',
  'lucide:server',
  'lucide:wifi',
  'lucide:shopping-cart',
  'lucide:user',
  'lucide:briefcase',
  'lucide:house',
  'lucide:phone',
  'lucide:laptop',
  'lucide:cloud',
  'lucide:database',
  'lucide:lock',
  'lucide:id-card',
  'lucide:file-text',
  'lucide:terminal',
  'lucide:gamepad-2',
  'lucide:plane',
  'lucide:heart-pulse',
  'lucide:graduation-cap',
] as const

/** Shown when an entry has no icon or an unknown one. */
export const DEFAULT_ENTRY_ICON = 'lucide:key-round'

/** The colors of the picker (data, not theme: the user's choice for an entry). */
export const ENTRY_COLORS = [
  '#ef4444',
  '#f97316',
  '#eab308',
  '#22c55e',
  '#14b8a6',
  '#3b82f6',
  '#8b5cf6',
  '#ec4899',
  '#78716c',
] as const

export function isKnownIcon(name: string | null | undefined): boolean {
  return (
    typeof name === 'string' &&
    (ENTRY_ICONS as readonly string[]).includes(name)
  )
}
