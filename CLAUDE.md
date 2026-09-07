<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan
<!-- SPECKIT END -->

## Manifest precedence

When `.haex-hive.json` and `.spaex.json` coexist, `.haex-hive.json` and its
pinned source revisions are authoritative for the effective constitution.
`.spaex.json` is retained as a Spaex-compatible projection and MUST use pins
valid for Spaex's publisher schema; the two manifests MUST NOT be reconciled by
mixing or choosing between their revisions. If their resolved content differs,
follow `.haex-hive.json` and raise the drift for review.
