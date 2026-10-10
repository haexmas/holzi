# Public test signing keys are not key material

Status: accepted
Date: 2026-10-10

## Context

Constitution principle I forbids committing "SSH private keys, OAuth tokens, API keys, passwords,
encrypted secret blobs, and other key material". Since spec 017, the e2e extension bundles
(`src-tauri/tests/fixtures/extension_e2e/*.xt`) are signed with an Ed25519 key derived from a public seed
string in `src-tauri/tests/extension_e2e_fixtures.rs`. Spec 018 adds one more such bundle (the test
extension with agent tools). Read literally, a key derivable from committed text is key material.

## Decision

**A signing key for test scenarios may be derived from a public seed in the repository.** Such a key:

- signs only fixtures in this repository and protects nothing; anyone may sign with it;
- names its purpose in the seed text ("… fixtures only, never use for real signing …");
- is never a publisher key of a real extension, never added to any trust list, and never used outside
  tests.

Principle I keeps its full force for every key that protects something: real publisher keys, provider
credentials, vault keys, device keys.

## Consequences

- `extension_e2e_fixtures.rs` (017) and the agent-tools fixture of spec 018 follow this ADR.
- A new test key follows the same pattern: a seed string that says it is for tests only.
- The operator confirmed this on 2026-10-10 during the analysis of spec 018.
