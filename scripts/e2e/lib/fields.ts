import type { FlowInstance } from './flows.ts'

// Helpers for haex-ui's fields (spec 035-appearance-and-fields, FR-002 to FR-004). A field is found by
// the `id` its `<input>` carries; the border is its input group (or the textarea frame).

const GROUP = `el.closest('[data-slot="input-group"]') ?? el.parentElement`

/** Whether the label of the field with this id sits on the border (floated) instead of inside. */
export async function labelFloated(
  instance: FlowInstance,
  id: string,
): Promise<boolean> {
  return instance.exec<boolean>(
    `const el = document.getElementById(arguments[0])
     const label = document.querySelector('label[for="' + arguments[0] + '"][data-slot="floating-label"]')
     if (!el || !label) throw new Error('no field or label for ' + arguments[0])
     const frame = (${GROUP}).getBoundingClientRect()
     const box = label.getBoundingClientRect()
     return box.top + box.height / 2 <= frame.top + 4`,
    [id],
  )
}

/** Whether the border of the field with this id has the colour of `--primary` (the focus ring, FR-003). */
export async function borderIsPrimary(
  instance: FlowInstance,
  id: string,
): Promise<boolean> {
  return instance.exec<boolean>(
    `const el = document.getElementById(arguments[0])
     if (!el) throw new Error('no field for ' + arguments[0])
     const probe = document.createElement('div')
     probe.style.borderTop = '1px solid var(--primary)'
     document.body.appendChild(probe)
     const primary = getComputedStyle(probe).borderTopColor
     probe.remove()
     return getComputedStyle(${GROUP}).borderTopColor === primary`,
    [id],
  )
}

/** The error text tied to the field with this id, or null (FR-004): the paragraph the input names in
 * `aria-describedby`, only while the input is marked invalid. */
export async function fieldError(
  instance: FlowInstance,
  id: string,
): Promise<string | null> {
  return instance.exec<string | null>(
    `const el = document.getElementById(arguments[0])
     if (!el || el.getAttribute('aria-invalid') !== 'true') return null
     const ids = (el.getAttribute('aria-describedby') ?? '').split(' ').filter(Boolean)
     for (const id of ids) {
       const node = document.getElementById(id)
       if (node && node.getAttribute('role') === 'alert') return node.textContent.trim()
     }
     return null`,
    [id],
  )
}

/** Selector of the clear button of the field with this `data-testid` (haex-ui gives the button no hook of
 * its own; it follows the input inside the field's group). */
export const clearButtonOf = (hook: string): string =>
  `[data-testid="${hook}"] ~ button`
