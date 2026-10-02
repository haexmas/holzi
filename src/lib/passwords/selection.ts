// Multi-selection of entries (spec 034-password-manager, US2, FR-012): ctrl or long press toggles,
// shift selects the range from the anchor over the entries as the list shows them. Pure and
// immutable, so `scripts/check-passwords-selection.ts` runs it without vue.

export type SelectionState = {
  /** Selected ids in the order they were added. */
  selected: readonly string[]
  /** Where a shift range starts: the entry that was toggled or picked last. */
  anchor: string | null
}

export function emptySelection(): SelectionState {
  return { selected: [], anchor: null }
}

export function isSelected(state: SelectionState, id: string): boolean {
  return state.selected.includes(id)
}

/** Adds the entry, or removes it if it was selected; it becomes the anchor. */
export function toggle(state: SelectionState, id: string): SelectionState {
  return isSelected(state, id)
    ? {
        selected: state.selected.filter((selected) => selected !== id),
        anchor: id,
      }
    : { selected: [...state.selected, id], anchor: id }
}

/** Selects the entries from the anchor to `to` (both included, either direction) over `visible`,
 * keeping what was selected. Without an anchor, or when one of the two is not visible, it selects
 * just `to`. */
export function selectRange(
  state: SelectionState,
  visible: readonly string[],
  to: string,
): SelectionState {
  const from = state.anchor === null ? -1 : visible.indexOf(state.anchor)
  const end = visible.indexOf(to)
  if (end === -1) return state
  if (from === -1) return toggleOn(state, to)
  const [low, high] = from <= end ? [from, end] : [end, from]
  const added = visible.slice(low, high + 1)
  const selected = [...state.selected]
  for (const id of added) if (!selected.includes(id)) selected.push(id)
  return { selected, anchor: state.anchor }
}

function toggleOn(state: SelectionState, id: string): SelectionState {
  return {
    selected: state.selected.includes(id)
      ? state.selected
      : [...state.selected, id],
    anchor: id,
  }
}

export function selectAll(visible: readonly string[]): SelectionState {
  return { selected: [...visible], anchor: visible.at(-1) ?? null }
}

/** Drops ids that no longer exist (an entry deleted elsewhere, a filter that hides it). */
export function pruneSelection(
  state: SelectionState,
  existing: ReadonlySet<string>,
): SelectionState {
  const selected = state.selected.filter((id) => existing.has(id))
  if (selected.length === state.selected.length) return state
  return {
    selected,
    anchor:
      state.anchor !== null && existing.has(state.anchor) ? state.anchor : null,
  }
}
