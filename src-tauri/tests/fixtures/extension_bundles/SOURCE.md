# Source of the bundle test vectors (spec 017, T012)

Copied unchanged from [haex-space/vault-sdk](https://github.com/haex-space/vault-sdk) at
`36bf6e98f36c2362d42aa2d92c85288a3d91e775` (release 4.0.0, after haex-space/vault-sdk#54), path
`test-vectors/bundles/`. `README.md` and `expected.json` are part of the copy.

The vectors are generated there by `scripts/generate-bundle-vectors.mjs`; never edit them here. To
update, regenerate in vault-sdk, merge, copy the folder again and change the SHA above.

Every vector is signed with public test keys whose private halves are derived from public seeds (see
`README.md`). They are no secret and must never be trusted outside these tests.
