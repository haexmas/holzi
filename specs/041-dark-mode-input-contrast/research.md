# Research: Dark-Mode-Input-Kontrast

## Decision: Adjust the existing Dark Mode input token

The shared stylesheet defines `--input` and `--border` for both color schemes,
and the base layer applies the border token to all elements. In Dark Mode both
tokens currently resolve to the same dark neutral value, so an input border has
little separation from its surrounding surface. A slightly lighter `--input`
value gives non-focused inputs a visible boundary while leaving the surrounding
border token and Light Mode values unchanged. The matching Dark Mode default in
`src/lib/appearance/tokens.ts` must change with it because the appearance
derivation check intentionally keeps that source aligned with the stylesheet.

## Measured contrast

WCAG 2 contrast ratios of the `--input` border against the surfaces it sits
on, computed with `src/lib/appearance/contrast.ts`:

| Scheme        | `--input` | background | card, popover | sidebar | muted |
| ------------- | --------- | ---------- | ------------- | ------- | ----- |
| Light         | 0.922     | 1.10       | 1.22          | 1.10    | 1.15  |
| Dark (before) | 0.269     | 1.19       | 1.19          | 1.19    | 1.00  |
| Dark (after)  | 0.35      | 1.58       | 1.58          | 1.58    | 1.34  |

Before the fix the dark border was weaker than the light one on the card and
invisible on muted; after it, the dark border exceeds the light one on every
surface. In Dark Mode the input fill (`--input` at 30 % over the background)
stays at 1.12:1 against the background, and text keeps 15.3:1 (foreground)
and 6.2:1 (muted foreground) on it.

Neither scheme reaches the 3:1 that WCAG 1.4.11 asks of a component
boundary. That needs about 0.54 in Dark Mode and about 0.63 in Light Mode and
would also restyle outline buttons, checkboxes, selects and tabs that share
the token; it changes Light Mode, which FR-002 keeps, so it is left as a
follow-up decision rather than part of this fix.

## Rationale

- The symptom is global and is controlled by the existing theme tokens and
  their shared appearance default.
- A token-level change reaches all common input instances consistently.
- Focus styles remain controlled by their existing ring/focus rules.
- No component duplication, new selector, or dependency is needed.

## Alternatives considered

- **Add borders to individual Vue components**: rejected because it would miss
  other inputs and create inconsistent styling.
- **Change the Dark Mode background surface**: rejected because it would alter
  the broader application palette rather than the affected control boundary.
- **Change the Light Mode token**: rejected because Light Mode already has the
  requested contrast.

## Open questions resolved

There are no remaining implementation unknowns. The existing theme token and
base border rule provide the correct shared seam for this fix.
