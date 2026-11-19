# Case study template

> **Status:** Template, no case study published
> **Date:** 2026-11-08
> **Rules:** [`program.md`](program.md); consent and redaction (CON-501, CON-519)

Copy this file, replace every `<placeholder>`, and publish only when the consent
statement below is signed and the redaction checklist is complete. A case study
that skips either is not published, and no partner is named without written
permission. Being listed in [`tracker.md`](tracker.md) is not that permission.

## Consent statement

> [to be confirmed]
>
> The partner named in this case study has given **written approval** for:
>
> 1. the use of their project and organisation name in a public document;
> 2. the publication of the measured outcomes below;
> 3. quotation of the attributed remarks in this document.
>
> Approval was given by `<role>` at `<project-name>` on `<YYYY-MM-DD>`, and a copy is
> kept in the private partner record. Removal is honoured on request
> (`program.md`).

- Consent given by (role, not name): `<role>`
- Date of consent: `<YYYY-MM-DD>`
- Consent covers: `<name, outcomes, quotes>`
- Private evidence reference: `<link>`

## Case study: `<project-name>`

| Field | Value |
|---|---|
| Project | `<project-name>` |
| Sector | `<sector>` |
| Crates in use | `<crate>` |
| Version at writing | `<version>` |
| In production since | `<YYYY-MM-DD>` |
| Maintainer contact role | `<role>` |

### The problem

`<two to four sentences: the integration, the volume, and the constraint that made
this a real problem rather than a hypothetical one>`

### The integration

`<the crates and surfaces used, and why; what was rejected and why>`

### Measured outcomes

Numbers, not adjectives. A row holding `<not measured>` or `[to be confirmed]` is
honest. A row holding "much faster" is not, and does not belong here.

| Measure | Before | After | Method / source |
|---|---|---|---|
| `<measure>` | `<value>` | `<value>` | `<how it was measured>` |
| `<measure>` | `<value>` | `<value>` | `<how it was measured>` |
| `<measure>` | `<value>` | `<value>` | `<how it was measured>` |

### What worked

`<specific, checkable statements about the API or behaviour>`

### What did not

`<the honest list. Mandatory. "None found" is acceptable only when no rough edge
was reported>`

### Attributed remarks

> `<quote>`
>
> - `<role>`, `<project-name>`

## Redaction checklist

Complete every line before publishing. An unchecked line keeps the study private.

- [ ] No customer names, end-user names, account IDs, or email addresses.
- [ ] No customer data, screenshots, document contents, or sample payloads.
- [ ] No credentials, API keys, tokens, webhook URLs, or private endpoints.
- [ ] No internal hostnames, IP addresses, or unflagged infrastructure details.
- [ ] No revenue, pricing, contract, or commercially sensitive figures.
- [ ] Every number carries its method or source in the table above.
- [ ] Every quote is attributed by role only, and the partner approved the wording.
- [ ] Placeholders `<...>` are replaced, and `[to be confirmed]` is resolved or the
      row deleted.
- [ ] The partner reviewed this file end to end after redaction.
- [ ] Removal is honoured on request, and the tracker row is set to `declined`.

## Publication

- Publication date: `<YYYY-MM-DD>`
- Published at: `<url>`
- Tracker status set to `case study published` on: `<YYYY-MM-DD>`