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

- 2026-09-28, operator decisions D26 to D31 (identity model v2) replace D8 and the rotation parts of
  D23; they supersede every earlier note about identity rotation, lock-out by a new vault identity,
  per-device attestations and closing or leaving shared scopes on rotation. Those notes are removed.
- There is no vault identity rotation in v1 (D26). Only main devices (Hauptgeräte) hold the vault
  identity private key and sign the device list (Geräteliste, FR-005), which replaces per-device
  attestations everywhere; several main devices are allowed (D27). Linked devices (verknüpfte
  Geräte) read and write all vault data and may manage spaces and data shares (D29, FR-040).
- User stories: US1 data sync (P1), US2 only own devices (P1), US3 gap-free progress and authorship
  (P1), US4 Föderation → Geräte with roles and the read-only public vault identity (P2, FR-033 to
  FR-035, new FR-046), US5 link a device (P1, FR-023 to FR-025; role question, default linked device,
  total-loss warning), US6 remove a device (P2, FR-026 to FR-028: new device list with a cutoff
  „Grenze beim Entfernen“ plus a new generation of the vault content key), US7 copy of the vault file
  as a secondary way (P2, new FR-044 „Kopie der Vault-Datei“ and FR-045 „Aufnahmeanfrage“).
- FR-039 is now „Umschläge an jedes Gerät“ (D28, own side: the vault stores received keys; SC-019),
  FR-040 „Jedes Gerät der Admin-Vault verwaltet“ (D29), FR-041 „Admin-Rolle nicht übertragbar“ (D23,
  without rotation). FR-038 „Nur-direkt-Daten“ now covers only the vault identity private key (D30;
  SC-012). SC-013 now tests device-list integrity and same-generation merges. Recovery is spec 026
  (D31).
- Kept mechanisms: package atomicity and per-group snapshot checks (FR-013, SC-017), gap-free
  progress (FR-019, SC-014), revocation cutoff (FR-042, SC-015), same-generation lists (FR-043,
  SC-016, now also for device lists), one-device presence bootstrap adapted to the device list
  (FR-007, SC-002, SC-007).
- iroh, Nostr, secp256k1 and NIP-44 are operator-mandated constraints from the design document;
  they are named in the Input, Begriffe and Assumptions. The requirements describe observable
  behaviour and security properties. The "no implementation details" items are ticked on that basis.
- The linking flow replaces the unbuilt "Verbinden" flow of spec 001 (user story 2, FR-015 to
  FR-017 there); the main-device role corresponds to `pairing-authority` of v1-scope §4, while
  `peer_instances` and `confirmation-authority` are out of scope.
- "Bereich" (scope) collides with the "Bereich / Unteransicht" of settings categories in spec 023;
  the Begriffe section disambiguates it.
- SC-011 (multi-process end-to-end tests) depends on the spec 016 rig being able to run several app
  processes; the plan confirms this.
- All clarification markers are resolved and recorded under Clarifications; superseded answers are
  marked there.
- 2026-09-28, plan review with the operator: within the vault scope there are no stored change
  packages and no sequence numbers. Own devices exchange transaction groups from their current
  state over the verified iroh connection; progress is the highest HLC per origin device, whose id
  is already in every HLC (FR-012, FR-013, FR-019 to FR-021, FR-028, SC-004, SC-014). Change
  packages remain for paths through third parties (026) and sequence numbers remain for shared
  scopes (027, 028; FR-019, FR-042), so the references from those specs stay valid. A copy of the
  vault file deletes nothing (FR-006). "Relay" is never used alone: Nostr-Relay, iroh-Relay or
  Sync-Server (026, formerly "das Relay").
- 2026-09-28, second plan review with the operator: a device list never ends up without a main
  device; among valid same-generation lists the smallest-hash list is authoritative for devices
  and removals, and a next-generation merge carries those removals forward. `issued_by` cannot be
  made forgery-proof (all main devices share the vault identity key) and is informational only.
  Columns overwritten later are delivered as the subset that still holds, as haex-crdt does today.
  Writes move to `execute_with_crdt` via a small haex-crdt extension (plan).
