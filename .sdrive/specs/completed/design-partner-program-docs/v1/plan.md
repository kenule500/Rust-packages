---
title: Design partner program and case-study docs plan
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# Design partner program and case-study docs plan

## Approach

Add three Markdown documents under `docs/design-partners/` and link them from the
README. The program page defines eligibility, obligations, the production
definition, and the G5 gate. The tracker is an empty template with fixed columns
and no invented partner. The case-study template carries consent, a redaction
checklist, and a measured-outcomes section. A small Node check validates the
required sections and the absence of placeholder partners, so the documents cannot
quietly lose the rules that make them useful.

## Components

| Component | Action | Notes |
|---|---|---|
| `docs/design-partners/program.md` | create | Eligibility, benefits, obligations, production definition, G5 gate. |
| `docs/design-partners/tracker.md` | create | Empty template with seven columns; no fabricated partner. |
| `docs/design-partners/case-study-template.md` | create | Consent, redaction checklist, measured outcomes. |
| `scripts/docs-partners.js` | create | Validates required sections, the tracker columns, and that no placeholder row claims a partner. |
| `README.md` | modify | Link to the program page. |

## Data model

No runtime data. The tracker is a Markdown table whose columns are: project,
contact role, crates in use, environment, start date, status, and evidence link.
Status values are `prospect`, `active`, `case study published`, `declined`.

**Flow.** The maintainer adds a row only after a partner consents, records an
evidence link, and marks status. A case study is drafted from the template, redacted
against the checklist, and published only with written approval.

## Security considerations

- **CON-501** — no customer data without redaction; the template's checklist is mandatory.
- **CON-519** — no credentials or private contact details; the tracker records roles, not people.
- **CON-704** — every partner fact is sourced or marked as a placeholder.

## Performance considerations

- Not applicable: static Markdown and one small Node check.

## Dependencies

| Dependency | Type | Action | Notes |
|---|---|---|---|
| `node` | external | no_change | Already present for `scripts/`. |

## Risks

- **RISK-001** — adoption may not materialise; the feature makes the requirement auditable, not guaranteed.
- **RISK-002** — a thin documentation feature can become a filing exercise; the empty-template and sourcing rules keep it honest.
- **RISK-003** — partner data is personal; roles plus redaction rules are the mitigation.
