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

- [ ] No [NEEDS CLARIFICATION] markers remain
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

- Two clarification markers remain open, both taken from the design's open
  questions (§15):
  - FR-022: who may upload a Momentaufnahme — any member with Schreiben, or
    only the admin of the Bereich (§15 item 5). For the Bereich "Vault" both
    answers mean every own device. FR-042 (re-upload of own changes missing
    after a snapshot) limits the damage of an incomplete snapshot either way.
  - FR-040: whether one Bereich may live on several relays at once for
    redundancy (§15 item 6). The marker states the fallback: without
    redundancy the vault's Postfach lives on the first configured relay. This
    is the single shared question for 026 and 027; Spec 027 references it
    instead of asking it separately.
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
- Cross-spec alignment (2026-09-28): FR-021 accepts a hand-over signed by the
  old Vault-Identität after a rotation and freezes the Bereich on competing
  hand-overs (Spec 024 owns the rotation); FR-021 also carries Zulassung and
  Kontingent over with the hand-over (addition, flagged for review); how a
  frozen Bereich is released is left to the plan with Spec 024. FR-032 cites
  "Spec 024, Nur-direkt-Daten" and allows envelopes and the Dateiindex in the
  vault's Postfach. FR-037 follows 024's atomicity (package whole,
  Momentaufnahme per change). FR-028 names admin garbage collection. FR-009
  keeps signalling optional, invite transport belongs to 027. FR-038 names the
  sub-view „Relays“ of „Föderation“. New FR-048: own synced folders (025) may
  store their Objekte on Speicher-Backend A. Terminology: Fortschrittsstand
  instead of Versionsvektor, „Admin“ instead of "Ersteller" for the Bereich
  creator.
- Default values (list lifetime 90 days, renewal at half, 80 % quota warning,
  24 h withholding deadline, 5 min clock tolerance) are assumptions for the
  plan to confirm.
