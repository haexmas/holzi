# Specification Quality Checklist: Blind Relay: Sync über einen nicht vertrauenswürdigen Server

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

- The clarification history records two questions that are now resolved: FR-022
  limits snapshot uploads to the admin of the Bereich, and FR-040 selects one
  home relay per Bereich in v1. Spec 027 uses the same relay decision.
- "No implementation details": iroh, NIP-42, Nostr and S3 appear only in the
  Input line and in the Assumptions, as operator constraints from the design.
  FR-024 names secp256k1 because the operator's rationale for dropping UCAN
  (one key system shared with Nostr) is a requirement, not a technology
  choice. The haex-sync-server file paths in "Beziehung zu bestehenden Specs"
  are lessons, pinned by repository and full SHA, not requirements.
- Additions beyond the design, flagged for review: FR-021 binds the Bereich id
  to the admin's Vault-Identität so the relay can check list signatures
  without trust on first use; FR-028 lets the admin delete objects in
  addition to uploader and Löschen holders and counts all storage of a
  Bereich against the admin's quota; FR-006 lets members use foreign Bereiche
  without their own admission; FR-039 pins the relay's identity; FR-020
  carries over the minimum-generation check from the deferred design §4.
- Cross-spec alignment (2026-09-28): FR-021 lets the admin end a Bereich
  with a signed, final Ende-Erklärung and never binds a Bereich to another
  Vault-Identität (earlier hand-over and identity-change content removed, D26).
  FR-032 names the private key of the Vault-Identität as the only
  Nur-direkt-Datum (D30) and allows envelopes, received Space keys, S3
  credentials and the Dateiindex in the vault's Postfach. FR-037 follows 024's atomicity (package whole,
  Momentaufnahme per change). FR-028 names admin garbage collection. FR-009
  keeps signalling optional, invite transport belongs to 027. FR-038 names the
  sub-view „Relays“ of „Föderation“. New FR-048: own synced folders (025) may
  store their Objekte on Speicher-Backend A. Terminology: Fortschrittsstand
  instead of Versionsvektor, „Admin“ instead of "Ersteller" for the Bereich
  creator.
- Default values (list lifetime 90 days, renewal at half, 80 % quota warning,
  24 h withholding deadline, 5 min clock tolerance) are assumptions for the
  plan to confirm.

- 2026-09-28: the operator answered the several-relays question (FR-040: one home relay per Bereich
  in v1) and the snapshot-upload question (FR-022: only the admin of the Bereich). No markers remain.
- 2026-09-28 (D24): Speicher-Backend A has no presigned links anymore; the relay streams the encrypted Objekte itself over its own endpoint (FR-026), so a removal takes effect immediately for objects too (FR-023, SC-003, SC-009 rewritten; key entity „Zugangslink“ removed).
- 2026-09-28 (D26–D32, identity model v2): no change of the Vault-Identität
  (D26); the relay admits a device only if it is on the vault's current
  Geräteliste and keeps the newest Geräteliste per vault (FR-004, FR-049,
  D27); lists and Ende-Erklärungen are signed by any device on the admin's
  Geräteliste (FR-018, FR-021, D29); explicit relay roles (SQLite data in
  Postfächer only, optional Speicher-Backend A streamed by the relay, each
  Objekt stored once; FR-026, FR-027, D32). New User Story 9 (P2) and
  FR-050 to FR-056, SC-013 to SC-015 for recovery via an encrypted
  Wiederherstellungspaket (D31). Additions flagged for review: the package is
  encrypted to a public key derived from the Wiederherstellungsschlüssel, so
  Hauptgeräte can refresh it without keeping the key (FR-052); only a
  Hauptgerät may upload it; the recovery code embeds the relay address
  (FR-050); the relay does not reveal whether a package exists before the
  possession proof (FR-053); recovery defaults (128 bit, TOTP 6 digits/30 s,
  e-mail link 15 min, lockout after 5 failures, doubling) are assumptions.
