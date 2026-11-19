# Design partner tracker

> **Status:** Empty template
> **Date:** 2026-11-08
> **Rules:** [`program.md`](program.md) defines eligibility, the 30-day production test, and the G5 gate.

This table is the record of design-partner conversations. **No partner is listed
yet**, so every row holds `<placeholder>` values. A row is added only after a
person at the partner organisation has agreed in writing to the program's
obligations; the placeholder rows show the shape of a finished row, not a
conversation that is further along than it is.

The table records a contact **role**, never a person: no names, email addresses,
phone numbers, or company handles belong here (CON-501, CON-519).

## Partners

| Project | Contact role | Crate in use | Environment | Start date | Status | Evidence link |
|---|---|---|---|---|---|---|
| `<project-name>` | `<role>` | `<crate>` | `<production / staging / dev>` | `<YYYY-MM-DD>` | `prospect` | `<link>` |
| `<project-name>` | `<role>` | `<crate>` | `<production / staging / dev>` | `<YYYY-MM-DD>` | `prospect` | `<link>` |
| `<project-name>` | `<role>` | `<crate>` | `<production / staging / dev>` | `<YYYY-MM-DD>` | `prospect` | `<link>` |

## Column and status meaning

| Column | Meaning |
|---|---|
| Project | The partner's project or organisation name, if they consented to be named. Otherwise `<private>`. |
| Contact role | A role such as `maintainer`, `engineer`, or `lead`; never a person's name. |
| Crate in use | `wirekit`, `dado-yaml`, `platform-core`, `quota-ledger`, `hookgate`, or `syncforge`. |
| Environment | `production` counts toward G5 only when it meets the 30-day live test in `program.md`; `staging` and `dev` do not. |
| Start date | First production deploy the owner can point at, in `YYYY-MM-DD`. |
| Status | `prospect`, `active`, `case study published`, or `declined`. |
| Evidence link | A URL or issue that proves the row: a public site, a release note, or a private reference for private partners. |

A row without an evidence link is incomplete and does not count toward the 1.0
gate. `prospect` rows are conversations, not evidence. Only `active` rows on a
`production` environment count toward G5, and G6 counts from GIA.

## G5 and G6 progress

| Gate | Needed | Counted | Note |
|---|---|---|---|
| G5 | three external projects in production | 0 | No row is `active` yet. |
| G6 | GIA in production | 0 | [to be confirmed] |

## Adding a row

1. Confirm the partner's written consent and which of the three listings they
   chose: private, listed, or case study.
2. Confirm the production claim against `program.md`: on crates.io, non-prerelease,
   real traffic, 30 consecutive days, owner confirmation.
3. Fill every column; leave nothing blank. Use `<private>` for a partner who asked
   not to be named.
4. Paste the evidence link and open a pull request. A row is merged only with a
   reviewer's approval, and the commit message names the gate it moves.

## Removing a row

Ask, and it is gone: delete the row, unpublish any public case study, and update the
counts above. Removal is honoured without a reason, no questions asked
(`program.md`, "Removal is honoured").