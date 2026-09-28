# Specification Quality Checklist: Datenfreigaben: Daten von Erweiterungen mit einzelnen Nutzern teilen

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

- Two clarification markers remain open, both taken from the proposed rules in
  the sync design §11 and §15 items 1 and 2:
  - FR-028, delete semantics: the spec is written assuming the proposal
    ("Schreiben" deletes own entries, deleting others' entries needs "Löschen").
  - FR-034, revocation vs. concurrent writes: the spec is written assuming the
    proposal (a change counts if its author held the capability in the key
    generation it is encrypted with, and every receiving device also checks the
    author against the newest member list it knows; the relay blocks at once).
  - Both markers use the unified texts shared with spec 027 (FR-016, FR-024
    there).
- Cross-spec alignment (2026-09-28): every membership change, including an
  invite, creates a new key generation, and new recipients get all older
  generations (FR-010, FR-011); capabilities are three nested levels as in 027
  (FR-008); member-list rules adopted from 027 (FR-016); unwrapped content keys
  are "Nur-direkt-Daten" per 024 (FR-022); package atomicity per 024 (FR-024);
  invite transport and direct connections between members come from 027;
  identity rotation is owned by 024 (edge cases); management lives in the
  settings category "Föderation", sub-view "Datenfreigaben" (FR-015).
- The declaration schema is described by the information it must carry (FR-001),
  not by syntax; the manifest format is plan work and touches `vault-sdk` and
  `haextension` as well.
- FR-041 (the owner vault re-issues a forwarded change as a new change signed
  by an owner device in the other share's Bereich; the original author is kept
  for display only) goes beyond design §9.2/§9.3: without it, the receiver-side
  capability check would reject every forwarded change, because its original
  author is not a member of the other share. It is an explicit exception to
  024's rule that a forwarder never changes author or signature, and FR-023 (e)
  exempts re-issued changes from "creator of a new row = author".
- FR-002 (d) and (e) (declared columns exist, FK points to the parent table,
  unique names, no cycles) add structural validity checks to the three D5
  rejection rules.
- Dependency: holzi has no extension-owned, prefixed, synced tables yet (design
  §15 item 10) and no extension host (planned spec 017). Stated in Assumptions;
  implementation waits for them and for specs 024 and 026 under the phasing
  rule.
- "Content quality" is checked with the caveat that the spec necessarily names
  protocol-level concepts (signatures, key generations, member lists) because
  the feature is a security boundary; no library, format or API is prescribed.

- 2026-09-28: the operator answered all clarification questions for this spec; the markers are
  resolved in the spec and recorded under Clarifications. Notes above that describe open markers are
  historical.
