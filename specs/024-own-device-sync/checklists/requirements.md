# Specification Quality Checklist: Vault-Identität, Geräteschlüssel und Datensync zwischen eigenen Geräten

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-09-28
**Feature**: [spec.md](../spec.md)

## Content Quality

- [x] No implementation details (languages, frameworks, APIs)
- [x] Focused on user value and business needs
- [x] Written for non-technical stakeholders
- [x] All mandatory sections completed

## Requirement Completeness

- [x] No [NEEDS CLARIFICATION] markers remain
- [x] Requirements are testable and unambiguous
- [x] Success criteria are measurable
- [x] Success criteria are technology-agnostic (no implementation details)
- [x] All acceptance scenarios are defined
- [x] Edge cases are identified
- [x] Scope is clearly bounded
- [x] Dependencies and assumptions identified

## Feature Readiness

- [x] All functional requirements have clear acceptance criteria
- [x] User scenarios cover primary flows
- [x] Feature meets measurable outcomes defined in Success Criteria
- [x] No implementation details leak into specification

## Notes

- Two clarification markers remain open for `/speckit-clarify`:
  - FR-004: whether vault copies made before this spec (placeholder identity)
    keep one shared identity derived from the placeholder, or each copy gets a
    fresh identity and becomes a separate vault.
  - FR-008: which servers carry presence and NAT traversal before the own
    relay (spec 026) exists: preset public Nostr/iroh relays, user-entered
    servers only, or local network only without a configured server. The
    same servers carry invitations to spaces and data shares (spec 027), so
    the answer applies to them too.
- iroh, Nostr and secp256k1 are operator-mandated constraints from the design
  document; they are named in the Input, Begriffe and Assumptions. The
  requirements describe observable behaviour and security properties
  (mutual attestation check, encrypted change packages on every link,
  allow-list for anything leaving the vault scope). The "no implementation
  details" items are ticked on that basis.
- Operator decisions D1, D8, D9, D11 and D13 are recorded as clarifications;
  the spec cut (§14 of the design) sets P1 for sync via copied vault files,
  P2 for pairing a fresh install and P3 for locking out a lost device.
- Lock-out acceptance on remaining devices (FR-027) requires a per-device user
  confirmation with a check code, because a stolen device holds the old vault
  identity (D8) and could sign any hand-over itself. This answers design §15
  item 4. Spec 024 is the single owner of vault identity rotation, also for
  shared scopes: FR-039 to FR-041 add the signed hand-over per space/data
  share, check-code confirmation by members, and the relay freezing a scope on
  competing hand-overs; specs 026–028 reference 024 for it (SC-013).
- Cross-spec resolutions applied: the Nur-direkt-Daten list (FR-038, tested by
  SC-012; FR-018 now treats the vault's own relay mailbox as scope „Vault“),
  package atomicity vs per-change snapshot checks (FR-013), the owner re-issue
  exception for forwarding between overlapping data shares (FR-021), the
  one-device rule limited to presence and direct sync in scope „Vault“
  (FR-007, SC-007), and the device list as sub-view „Geräte“ of „Föderation“
  (FR-033, FR-035).
- The pairing flow replaces the unbuilt "Verbinden" flow of spec 001 (user
  story 2, FR-015 to FR-017 there); `peer_instances` and the
  `pairing-authority`/`confirmation-authority` capabilities of v1-scope §4 are
  explicitly out of scope, since all devices of a vault are equal under D8.
- "Bereich" (scope) collides with the "Bereich / Unteransicht" of settings
  categories in spec 023; the Begriffe section disambiguates it.
- SC-011 (multi-process end-to-end tests) depends on the spec 016 rig being
  able to run several app processes; the plan confirms this.

- 2026-09-28: the operator answered all clarification questions for this spec; the markers are
  resolved in the spec and recorded under Clarifications. Notes above that describe open markers are
  historical.
- 2026-09-28, review of PR #155/#156: FR-019 now defines „lückenloser Fortschritt“ (gap-free
  per-origin sequence numbers next to the HLC, explicit gap requests, forgery rejection; SC-014).
  New FR-042 „Grenze beim Entzug“ (revocation cutoff by sequence number, no timestamp rule; SC-015,
  narrowed to the D19 window) and FR-043 „Mitgliederlisten gleicher Generation“ (smallest list hash
  wins; SC-016). FR-013 checks snapshots per complete transaction group (SC-017). FR-007 lets
  one-device vaults listen for presence so a fresh copy is found (SC-002, SC-007). FR-040/FR-041
  replaced the relay freeze by a majority rule (SC-013). The note above about the relay freezing a
  scope is historical.
- 2026-09-28, operator decision D23: no admin hand-over in v1. FR-039 „Schließen und Verlassen beim
  Rotieren“, FR-040 „Schließen ist endgültig“ and FR-041 „Admin-Rolle nicht übertragbar“ replace the
  hand-over, member acceptances and majority rule; User Story 6 scenarios 6–9 and SC-013 test the new
  model. The majority note above is historical.
