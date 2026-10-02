// The draft of the entry editor (spec 034, US1, research R7): the form state, and how it turns into
// a create input or into a partial update that sends only what the user changed. A secret that was
// not touched is not in the update at all, so the window never needs the stored password (FR-005).
// Pure, so `scripts/check-passwords-draft.ts` runs it without vue.

/** The part of an `ItemDetail` the form starts from (structural, so this module stays loadable
 * by the Node check scripts without the generated bindings). */
export type DetailLike = {
  title: string | null
  username: string | null
  url: string | null
  note: string | null
  icon: string | null
  color: string | null
  expiresAt: string | null
  tags: { name: string }[]
  keyValues: { id: string; key: string | null; hasValue: boolean }[]
}

/** Same shape as the generated `ItemInput`. */
export type InputOut = {
  title?: string
  username?: string
  password?: string
  note?: string
  url?: string
  icon?: string
  color?: string
  expiresAt?: string
  otpSecret?: string
  otpDigits?: number
  otpPeriod?: number
  otpAlgorithm?: string
  tags: string[]
  keyValues: { key: string; value?: string }[]
}

/** Same shape as the generated `ItemPatch`. */
export type PatchOut = {
  title?: string | null
  username?: string | null
  password?: string | null
  note?: string | null
  url?: string | null
  icon?: string | null
  color?: string | null
  expiresAt?: string | null
  otpSecret?: string | null
  otpDigits?: number | null
  otpPeriod?: number | null
  otpAlgorithm?: string | null
  tags?: string[]
  keyValues?: { id?: string; key: string; value?: string }[]
}

/** A custom field in the form. `value` is `null` while the stored value is untouched. */
export type KeyValueDraft = {
  id: string | null
  key: string
  value: string | null
  /** Whether the stored field has a value (only for fields that exist already). */
  hasStoredValue: boolean
}

/** What the user does to the TOTP of the entry. */
export type OtpDraft =
  | { mode: 'keep' }
  | { mode: 'clear' }
  | {
      mode: 'set'
      text: string
      digits: number | null
      period: number | null
      algorithm: string | null
    }

export type Draft = {
  title: string
  username: string
  url: string
  note: string
  icon: string | null
  color: string | null
  /** `YYYY-MM-DD` or empty. */
  expiresAt: string
  password: { mode: 'keep' } | { mode: 'set'; value: string }
  otp: OtpDraft
  /** Tag names. */
  tags: string[]
  keyValues: KeyValueDraft[]
}

export function emptyDraft(): Draft {
  return {
    title: '',
    username: '',
    url: '',
    note: '',
    icon: null,
    color: null,
    expiresAt: '',
    password: { mode: 'set', value: '' },
    otp: { mode: 'keep' },
    tags: [],
    keyValues: [],
  }
}

/** The form state of a stored entry: texts as they are, secrets untouched. */
export function draftFromDetail(detail: DetailLike): Draft {
  return {
    title: detail.title ?? '',
    username: detail.username ?? '',
    url: detail.url ?? '',
    note: detail.note ?? '',
    icon: detail.icon,
    color: detail.color,
    expiresAt: detail.expiresAt ?? '',
    password: { mode: 'keep' },
    otp: { mode: 'keep' },
    tags: detail.tags.map((tag) => tag.name),
    keyValues: detail.keyValues.map((field) => ({
      id: field.id,
      key: field.key ?? '',
      value: null,
      hasStoredValue: field.hasValue,
    })),
  }
}

/** A deep copy, so the initial state is not changed by edits. */
export function cloneDraft(draft: Draft): Draft {
  return JSON.parse(JSON.stringify(draft)) as Draft
}

export function isDirty(initial: Draft, draft: Draft): boolean {
  return JSON.stringify(initial) !== JSON.stringify(draft)
}

const orNull = (text: string): string | null => (text === '' ? null : text)

/** The input to create an entry: empty texts are left out, custom fields without a key too. */
export function toInput(draft: Draft): InputOut {
  const input: InputOut = {
    title: orNull(draft.title) ?? undefined,
    username: orNull(draft.username) ?? undefined,
    url: orNull(draft.url) ?? undefined,
    note: orNull(draft.note) ?? undefined,
    icon: draft.icon ?? undefined,
    color: draft.color ?? undefined,
    expiresAt: orNull(draft.expiresAt) ?? undefined,
    tags: [...draft.tags],
    keyValues: draft.keyValues
      .filter((field) => field.key.trim() !== '')
      .map((field) => ({ key: field.key, value: field.value ?? undefined })),
  }
  if (draft.password.mode === 'set' && draft.password.value !== '') {
    input.password = draft.password.value
  }
  if (draft.otp.mode === 'set' && draft.otp.text.trim() !== '') {
    input.otpSecret = draft.otp.text
    if (draft.otp.digits !== null) input.otpDigits = draft.otp.digits
    if (draft.otp.period !== null) input.otpPeriod = draft.otp.period
    if (draft.otp.algorithm !== null) input.otpAlgorithm = draft.otp.algorithm
  }
  return input
}

function sameList(a: readonly string[], b: readonly string[]): boolean {
  return a.length === b.length && a.every((value, i) => value === b[i])
}

/** The partial update from `initial` to `draft`: only what changed. A text that became empty is
 * set to empty (it is stored as it is), a date or an icon or a color that was removed is cleared. */
export function toPatch(initial: Draft, draft: Draft): PatchOut {
  const patch: PatchOut = {}
  if (draft.title !== initial.title) patch.title = draft.title
  if (draft.username !== initial.username) patch.username = draft.username
  if (draft.url !== initial.url) patch.url = draft.url
  if (draft.note !== initial.note) patch.note = draft.note
  if (draft.icon !== initial.icon) patch.icon = draft.icon
  if (draft.color !== initial.color) patch.color = draft.color
  if (draft.expiresAt !== initial.expiresAt)
    patch.expiresAt = orNull(draft.expiresAt)
  if (draft.password.mode === 'set') patch.password = draft.password.value
  if (draft.otp.mode === 'clear') {
    patch.otpSecret = null
  } else if (draft.otp.mode === 'set') {
    patch.otpSecret = draft.otp.text
    if (draft.otp.digits !== null) patch.otpDigits = draft.otp.digits
    if (draft.otp.period !== null) patch.otpPeriod = draft.otp.period
    if (draft.otp.algorithm !== null) patch.otpAlgorithm = draft.otp.algorithm
  }
  if (!sameList([...draft.tags].sort(), [...initial.tags].sort())) {
    patch.tags = [...draft.tags]
  }
  if (JSON.stringify(draft.keyValues) !== JSON.stringify(initial.keyValues)) {
    patch.keyValues = draft.keyValues.map((field) => ({
      id: field.id ?? undefined,
      key: field.key,
      // `undefined` keeps the stored value of a field that was not touched.
      value: field.value ?? undefined,
    }))
  }
  return patch
}
