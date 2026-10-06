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
