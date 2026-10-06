# Quickstart: Dark-Mode-Input-Kontrast

## Automated validation

From the repository root:

```sh
pnpm exec prettier --check src/assets/css/tailwind.css
pnpm build
```

## Visual validation

1. Open a representative form in Dark Mode.
2. Check one empty and one filled input without focus; both boundaries should
   be distinguishable from the surrounding dark surface.
3. Focus each input; the existing focus indicator should remain clear.
4. Switch to Light Mode and confirm the existing input appearance is unchanged.

## Expected result

The Dark Mode non-focused input boundaries are easier to identify, while Light
Mode and focused states retain their prior behavior.
