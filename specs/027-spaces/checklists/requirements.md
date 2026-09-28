# Specification Quality Checklist: Spaces: Netzwerkordner zum Teilen von Dateien mit anderen Nutzern

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

- Two clarification markers are open. Both use the unified texts shared with
  spec 028 (FR-028 and FR-034 there). The spec is written assuming the proposal
  in each:
  - FR-016, delete semantics: "Schreiben" deletes own files, deleting other
    members' files needs "Löschen".
  - FR-024, revocation vs. concurrent writes: a change counts if its author
    held the right in the key generation it is encrypted with and passes a
    check against the newest member list the receiving device knows; the relay
    blocks at once. A window remains for changes received before the
    revocation message.
    The Assumptions section names a residual gap in this rule (a removed member
    who still holds older keys can backdate a change); the operator decision
    should weigh it.
- The "several relays per Bereich" question is not asked here; the spec refers
  to spec 026 FR-040's marker.
- "All functional requirements have clear acceptance criteria" stays open until
  both markers are resolved; FR-016 and FR-024 depend on them.
- Cross-spec resolutions applied (coordinator brief, 2026-09-28):
  - Every member list change creates a new key generation with a fixed member
    list, invites included (FR-009, FR-019). New members also get envelopes for
    all older generations, so they read files from before they joined; rights
    in older generations do not follow from those envelopes.
  - New FR-039 "direkte Verbindung zwischen Mitgliedern": handshake with device
    key and device attestation of a vault on the current member list, one
    Bereich per connection, discovery via encrypted presence (spec 024 servers)
    or relay signalling. Specs 028 and 029 cite it.
  - New FR-040: the Postfach lives on the admin's relay independent of object
    storage; "nur direkte Übertragung" only concerns Objekte; without a relay
    the space syncs directly only.
  - New FR-041 "Zustellung von Einladungen": encrypted Nostr message to the
    invitee's Vault-Identität via spec 024's presence/signalling servers and
    the admin relay's signalling. Spec 028 cites it.
  - New FR-042 "Aufräumen alter Objekte": an admin device garbage-collects
    unreferenced Objekte (backend A via relay, spec 026 FR-028; backend B with
    admin credentials). FR-016 notes that members never delete objects
    themselves.
  - Identity model v2 (2026-09-28): no Vault-Identität rotation in v1 (D26);
    FR-043 now wraps every key generation to each device on the member
    vaults' current Gerätelisten (D28); any device of the admin vault may
    manage (FR-019, D29); direct connections and signatures are checked
    against the Geräteliste (FR-026, FR-039); relay syncs only Postfächer,
    files only via Speicher-Backend A or B (FR-040, D32).
  - FR-026 cites spec 024 for atomicity (package whole, Momentaufnahme per
    change). FR-029 uses the shared Konfliktkopie name pattern.
  - Management lives in settings category „Föderation“, sub-view „Spaces“; the
    storage backend is set in the space's view (FR-037). Backend switch comes
    with spec 029 User Story 8; the choice at creation is mandatory (FR-003).
- FR-022 forbids encrypting with a generation that is wrapped to a vault no
  longer on the merged member list. Without it, two admin devices
  concurrently creating generations (one removing a member) could leave the removed member able to
  read new content through the other device's generation.
- Operator constraints (iroh, Nostr, NIP-44 v2, encrypted Nostr direct message
  for invites) are named in the Input line and, for the invite transport, in
  FR-041. The haex-vault reference is given as repository, full SHA and path.
- Links to specs 024, 025, 026, 028 and 029 are plain references because those
  directories were written in parallel.

- 2026-09-28: the operator answered all clarification questions for this spec; the markers are
  resolved in the spec and recorded under Clarifications. Notes above that describe open markers are
  historical.
