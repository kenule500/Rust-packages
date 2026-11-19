---
title: Design partner program and case-study docs
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# Design partner program and case-study docs

## Overview

- **Problem Statement**: The 1.0 gate is adoption evidence — three external projects plus GIA in production (G5) — but the repository has no program, no tracker, and no case-study format. Adoption is also the item the roadmap calls the hardest part and the one most likely to be deferred to the end. Without a written program, "find design partners" stays a conversation and the 1.0 freeze has no evidence trail.
- **Proposed Solution**: Add a small, honest documentation set under `docs/design-partners/`: a program page defining eligibility, what a partner gets, what a partner owes, and how the 1.0 evidence gate is judged; a tracker page with an empty, clearly labelled template table; and a case-study template with consent and redaction rules. Link it from the README. Nothing is claimed about a partner who has not consented.
- **Target Audience**: The maintainer recruiting partners, and prospective partners deciding whether to join.
- **Assumptions**:
  - This is a documentation feature: no runtime code and no crate changes.
  - The tracker starts empty; real partners are added only after they consent in writing (CON-704: never invent project facts).
  - "In production" means a project running one of the crates in a live environment for at least 30 days, as the program page defines.
  - Publishing case studies is optional and needs written approval per partner.

## User Stories

### US-001: Recruit and track design partners honestly
As the maintainer, I want a written program and tracker, so that adoption work is repeatable and the 1.0 evidence gate has a record instead of a memory.

**Acceptance Criteria:**
- AC-001: Given `docs/design-partners/program.md`, when it is read, then it states eligibility, what a partner receives, what a partner commits to, and how "in production" and the three-project gate are judged.
- AC-002: Given `docs/design-partners/tracker.md`, when it is read, then it is an empty, labelled template with columns for project, contact, crate in use, environment, start date, status, and evidence link, and it contains no invented partner.
- AC-003: Given `docs/design-partners/case-study-template.md`, when it is read, then it includes a consent statement, a redaction checklist, and a section for measured outcomes rather than adjectives.
- AC-004: Given the repository README, when a developer looks for adoption information, then it links to the program page.
- AC-005: Given any page in the set, when it mentions a partner or a metric, then the claim is either sourced to a recorded evidence link or explicitly marked as a template placeholder.
- AC-006: Given the 1.0 decision gate, when the program page is read, then it names the G5 rule (three external projects plus GIA in production) and says that missing evidence blocks 1.0.

## Requirements

### Functional Requirements
- FR-001: `docs/design-partners/program.md` MUST exist and define eligibility, benefits, obligations, the production definition, and the G5 gate.
- FR-002: `docs/design-partners/tracker.md` MUST exist as a template with the seven named columns and MUST contain no fabricated partner.
- FR-003: `docs/design-partners/case-study-template.md` MUST exist with consent, redaction, and measured-outcome sections.
- FR-004: The README MUST link to the program page.
- FR-005: Every partner fact MUST be sourced or marked as a placeholder (CON-704).
- FR-006: Case studies MUST require written consent and MUST NOT include customer data without redaction (CON-501).
- FR-007: The documents MUST NOT contain credentials, private contact details, or unredacted customer data.
- FR-008: The program page MUST state that missing evidence blocks 1.0 rather than softening the gate.

### Non-Functional Requirements
- NFR-001: The pages MUST be Markdown, render on GitHub, and need no build step.
- NFR-002: The tracker MUST be usable without a database: a Markdown table is the source of truth until a partner consents to more.
- NFR-003: The documents MUST follow the repository's writing conventions and use placeholders such as `<project-name>`.

## UI/UX & Data Requirements

- **UI/UX**: Plain Markdown tables; no UI layer (CON-301).
- **Data**: Tracker columns — project, contact role (not personal data), crates in use, environment, start date, status (`prospect`, `active`, `case study published`, `declined`), evidence link.

## Edge Cases & Error States

- EC-001: A partner asks to be removed → the tracker row is deleted and the case study is unpublished; the program page says removal is honoured.
- EC-002: A partner will not consent to a case study → the row may still count toward G5 with a private evidence link, and no public study is written.
- EC-003: A partner has a dev-only integration → it does not count as production, and the program page says so.
- EC-004: Someone adds a partner without an evidence link → the template marks the row incomplete and the G5 rule refuses it.

## Out of Scope

- A CRM, signup form, or automation.
- Legal agreements; the program page names the topics an agreement must cover but is not the agreement.
- Publishing any case study before a partner consents.
- Changing the 1.0 gate itself.

## Dependencies & Risks

- No code dependency. Markdown only.
- **RISK-001**: Adoption is external and may not materialise; the feature makes the requirement visible and auditable, not guaranteed.
- **RISK-002**: A thin documentation feature can become a filing exercise; AC-002's empty template and AC-005's sourcing rule keep it honest.
- **RISK-003**: Partner data is personal; the tracker records roles rather than people and the case-study template mandates redaction (CON-501).

## Security Baseline

- CON-511 Authentication: NOT APPLICABLE — documentation only.
- CON-512 Authorization: NOT APPLICABLE — no tenant data.
- CON-513 Security Headers: NOT APPLICABLE — rendered by GitHub.
- CON-514 Rate Limiting: NOT APPLICABLE — no service.
- CON-515 Input Validation: NOT APPLICABLE — no runtime input.
- CON-516 Injection: NOT APPLICABLE — no database.
- CON-517 XSS: NOT APPLICABLE — no executable content.
- CON-518 CSRF: NOT APPLICABLE — no forms.
- CON-519 Environment Security: REQUIRED — no secrets or private contact details in the docs.
- CON-520 HTTPS: REQUIRED — every external link uses HTTPS.
