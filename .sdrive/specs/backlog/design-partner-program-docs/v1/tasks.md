---
title: Design partner program and case-study docs tasks
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# Design partner program and case-study docs tasks

> Documentation and one small checker only. Test authoring and execution belong to `/sdrive:test`.

## P1 - Program documents

- [ ] **T1.1 - Write the program page** (`FR-001`, `FR-008` / `AC-001`, `AC-006`)
  Add `docs/design-partners/program.md` defining eligibility, what a partner receives, what a partner commits to, what "in production" means (live for at least 30 days), and the G5 gate (three external projects plus GIA in production). State that missing evidence blocks 1.0.
- [ ] **T1.2 - Write the empty tracker template** (`FR-002`, `NFR-002` / `AC-002`, `AC-005`)
  Add `docs/design-partners/tracker.md` with the seven columns, the four status values, and no invented partner. Mark every cell that requires real data as a placeholder.
- [ ] **T1.3 - Write the case-study template** (`FR-003`, `FR-006` / `AC-003`)
  Add `docs/design-partners/case-study-template.md` with a written-consent statement, a redaction checklist, and a measured-outcomes section that requires numbers rather than adjectives.

## P2 - Wiring and verification

- [ ] **T2.1 - Link the program and add the checker** (`FR-004`, `FR-005`, `FR-007`, `NFR-003` / `AC-004`, `AC-005`)
  Link the program page from the README and add `scripts/docs-partners.js`, which asserts the three required sections, the tracker columns and status values, that placeholders use `<...>`, and that no row names a partner without an evidence link.
- [ ] **T2.2 - Run the verification checks** (`NFR-001`, `NFR-003` / `AC-001`, `AC-004`)
  Run `node scripts/docs-partners.js` and `node .sdrive/scripts/validate-governance.js`, and confirm the three pages render on GitHub without a build step.
