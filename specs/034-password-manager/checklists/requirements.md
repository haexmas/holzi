# Specification Quality Checklist: Passwortmanager

**Purpose**: Validate specification completeness and quality before proceeding to planning
**Created**: 2026-10-02
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

- Geklärt: FR-020 (Größenlimit pro Anhang) steht auf 25 MiB (Betreiber, 2026-10-02).
- Bewusst genannt, weil Betreiber-Vorgabe: die Tabellennamen `haex_passwords_*` (FR-036), der
  Bruch bei den Binärdaten (BLOB statt Base64-Text) sowie die Formate kdbx, Bitwarden, LastPass und
  die Hash-Art SHA-256 (Eindeutigkeit der Binärdaten). Diese sind Gegenstand der Anforderung, keine
  Wahl der Umsetzung.
- Für den Plan: Prüfen, dass das Präfix `haex_` in haex-crdt keine Sonderbehandlung auslöst;
  Aufruferkennung und Freigabe-Schnittstelle (FR-030) so schneiden, dass Specs 017–019 und 021 sie
  ohne Umbau nutzen; ADR nötig für die Entscheidung „Geheimnisse unverschlüsselt in der Vault,
  Schutz durch Freigaben“ (Constitution: Entscheidungen, die ein Kernprinzip berühren).
