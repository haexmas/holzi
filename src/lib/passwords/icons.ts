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

/** The pictures the import maps the KeePass standard icons to (`import/icons.rs`). They are not in
 * the picker, but an entry may hold them, so they are literals here for the icon scan. */
export const IMPORT_ICONS = [
  'lucide:apple',
  'lucide:award',
  'lucide:banknote',
  'lucide:bluetooth',
  'lucide:book-open',
  'lucide:calculator',
  'lucide:camera',
  'lucide:circle-check',
  'lucide:clipboard',
  'lucide:clipboard-check',
  'lucide:contact',
  'lucide:feather',
  'lucide:file-archive',
  'lucide:file-check',
  'lucide:file-lock',
  'lucide:file-pen',
  'lucide:file-plus',
  'lucide:files',
  'lucide:folder',
  'lucide:folder-archive',
  'lucide:folder-check',
  'lucide:folder-open',
  'lucide:globe',
  'lucide:hard-drive',
  'lucide:house',
  'lucide:id-card',
  'lucide:image',
  'lucide:key',
  'lucide:key-round',
  'lucide:key-square',
  'lucide:list',
  'lucide:lock-open',
  'lucide:mail',
  'lucide:mailbox',
  'lucide:message-circle',
  'lucide:monitor',
  'lucide:network',
  'lucide:nfc',
  'lucide:notebook',
  'lucide:notebook-text',
  'lucide:package',
  'lucide:pen',
  'lucide:plug',
  'lucide:printer',
  'lucide:puzzle',
  'lucide:scan',
  'lucide:screen-share',
  'lucide:server',
  'lucide:settings',
  'lucide:shield',
  'lucide:smartphone',
  'lucide:star',
  'lucide:tablet',
  'lucide:terminal',
  'lucide:timer',
  'lucide:triangle-alert',
  'lucide:user-key',
  'lucide:wallet',
  'lucide:wifi',
  'lucide:wrench',
  'lucide:zap',
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
    ((ENTRY_ICONS as readonly string[]).includes(name) ||
      (IMPORT_ICONS as readonly string[]).includes(name))
  )
}
