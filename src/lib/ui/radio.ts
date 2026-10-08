// Helpers for haex-ui's radio groups (`ShadcnRadioGroup`, reka-ui). Pure, no Vue imports.

/**
 * Whether the radio item a click landed on was already the checked one. A radio group never
 * clears itself, so a picker that lets a second click remove the choice asks this in its `click`
 * handler: the item's `data-state` still shows the state from before the click, because the group's
 * update re-renders only after the event.
 */
export function wasChecked(event: Event): boolean {
  return (
    (event.currentTarget as HTMLElement | null)?.dataset.state === 'checked'
  )
}
