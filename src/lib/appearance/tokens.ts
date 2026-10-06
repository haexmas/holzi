// The CSS variables the appearance settings set, which control sets which, and the pairs whose
// contrast is guaranteed (spec 035-appearance-and-fields, contracts/token-map.md). Pure.
import type { Control } from './schema.ts'

export type Scheme = 'light' | 'dark'

export const TOKEN_CONTROL = {
  primary: 'accent',
  'primary-foreground': 'accent',
  ring: 'accent',
  'sidebar-primary': 'accent',
  'sidebar-primary-foreground': 'accent',
  'sidebar-ring': 'accent',
  background: 'window',
  card: 'container',
  popover: 'container',
  sidebar: 'container',
  secondary: 'component',
  muted: 'component',
  accent: 'component',
  input: 'component',
  border: 'component',
  'sidebar-accent': 'component',
  'sidebar-border': 'component',
  foreground: 'text',
  'card-foreground': 'text',
  'popover-foreground': 'text',
  'secondary-foreground': 'text',
  'accent-foreground': 'text',
  'sidebar-foreground': 'text',
  'sidebar-accent-foreground': 'text',
  'muted-foreground': 'text',
} as const satisfies Record<string, Control>

export type TokenName = keyof typeof TOKEN_CONTROL
export const TOKEN_NAMES = Object.keys(TOKEN_CONTROL) as TokenName[]

/** The name of the CSS variable of a token. */
export const cssVar = (name: TokenName) => `--${name}`

/** Tokens the tints set; the accent tokens are derived from the surfaces instead. */
export type TintToken = {
  [Name in TokenName]: (typeof TOKEN_CONTROL)[Name] extends 'accent'
    ? never
    : Name
}[TokenName]
export const TINT_TOKENS = TOKEN_NAMES.filter(
  (name): name is TintToken => TOKEN_CONTROL[name] !== 'accent',
)

/** Lightness of each tinted token without a tint (the grey defaults of `tailwind.css`). */
export const DEFAULT_LIGHTNESS: Record<Scheme, Record<TintToken, number>> = {
  light: {
    background: 0.955,
    card: 0.99,
    popover: 0.99,
    sidebar: 0.955,
    secondary: 0.97,
    muted: 0.97,
    accent: 0.97,
    input: 0.922,
    border: 0.922,
    'sidebar-accent': 0.97,
    'sidebar-border': 0.922,
    foreground: 0.145,
    'card-foreground': 0.145,
    'popover-foreground': 0.145,
    'secondary-foreground': 0.205,
    'accent-foreground': 0.205,
    'sidebar-foreground': 0.145,
    'sidebar-accent-foreground': 0.205,
    // 0.556 reaches only 4.15:1 against the background; 4.5:1 is the limit (research R4).
    'muted-foreground': 0.53,
  },
  dark: {
    background: 0.205,
    card: 0.205,
    popover: 0.205,
    sidebar: 0.205,
    secondary: 0.269,
    muted: 0.269,
    accent: 0.269,
    input: 0.35,
    border: 0.269,
    'sidebar-accent': 0.35,
    'sidebar-border': 0.269,
    foreground: 0.985,
    'card-foreground': 0.985,
    'popover-foreground': 0.985,
    'secondary-foreground': 0.985,
    'accent-foreground': 0.985,
    'sidebar-foreground': 0.985,
    'sidebar-accent-foreground': 0.985,
    'muted-foreground': 0.708,
  },
}

/** Surfaces a control can sit on; text and controls are measured against each. */
export const SURFACES = [
  'background',
  'card',
  'popover',
  'sidebar',
  'muted',
] as const

/** Text pairs (foreground, background), at least 4.5:1. */
export const TEXT_PAIRS: readonly (readonly [TokenName, TokenName])[] = [
  ['foreground', 'background'],
  ['foreground', 'card'],
  ['foreground', 'popover'],
  ['foreground', 'sidebar'],
  ['foreground', 'secondary'],
  ['foreground', 'muted'],
  ['foreground', 'accent'],
  ['card-foreground', 'card'],
  ['popover-foreground', 'popover'],
  ['sidebar-foreground', 'sidebar'],
  ['secondary-foreground', 'secondary'],
  ['accent-foreground', 'accent'],
  ['muted-foreground', 'background'],
  ['muted-foreground', 'card'],
  ['muted-foreground', 'muted'],
  ['primary-foreground', 'primary'],
]

/** Control pairs (the control, the surface it sits on), at least 3:1. */
export const CONTROL_PAIRS: readonly (readonly [TokenName, TokenName])[] = (
  ['primary', 'ring'] as const
).flatMap((control) => SURFACES.map((surface) => [control, surface] as const))
