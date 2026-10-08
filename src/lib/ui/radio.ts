// Helpers for haex-ui's radio groups (`ShadcnRadioGroup`, reka-ui). Pure, no Vue imports.

/**
 * Lets a second click on the checked radio item clear the choice. A radio group never clears
 * itself: reka checks the clicked item again even when it is already checked. Its item emits a
 * cancelable `select` first, though; a picker passes it here from `@select`, and when the item was
 * the checked one the default is prevented (the group leaves its value alone) and `clear` runs.
 */
export function clearOnReselect(
  event: Event,
  checked: boolean,
  clear: () => void,
): void {
  if (!checked) return
  event.preventDefault()
  clear()
}
