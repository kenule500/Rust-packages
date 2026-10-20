# UI Context

> Context file for `.sdrive/constitution.md`.
> This repository ships libraries. It contains no user interface, no design
> tokens, and no Figma source.

## Observed Project Facts

### This repository has no UI layer

- `catalogue/` contains Rust crates and Markdown design docs only.
- There are no HTML, CSS, JavaScript, template, or asset files.
- The only non-Rust source is Markdown documentation.
- The README describes the project as public Rust packages, not an application:
  "Package docs, implementation, releases, and crates.io publishing happen here."
  Source: `catalogue/README.md`.

### UI obligations that belong to the consuming app

The provider docs describe behaviour the consuming product must implement. They
are not requirements on the libraries in this repository.

| Obligation | Owner | Source |
|---|---|---|
| Yelp attribution and branding in the UI | Consuming app | `catalogue/19_yelp.md` |
| Handle unverified Google Business locations during onboarding | Consuming app | `catalogue/06_google_business.md` |
| Do not argue with reviewers or include promotional links in review replies | Consuming app | `catalogue/06_google_business.md` |
| Show "3 changes pending" without exposing sync internals | Consuming app | `catalogue/02_syncforge.md` |
| Prompt for missing scopes instead of failing silently | Consuming app | `catalogue/28_google_oauth.md` |
| A disabled button is not an authorization check | Consuming app; the server is the enforcement point | `catalogue/35_studio_protocol.md` |
| Do not promise print/mail turnaround times | Consuming app | `catalogue/18_lob.md` |
| Do not promise real-time review events until verified | Consuming app | `catalogue/06_google_business.md` |

### Accessibility

- No accessibility tooling, lint rules, or component library exists in this
  repository.
- `studio-protocol` defines role-based permissions, but it does not define any
  accessible presentation of those controls.
  Source: `catalogue/35_studio_protocol.md`.

### Figma and design tokens

- No `figma.com` link appears in the repository.
- No design token JSON, colour palette, typography scale, spacing scale, or
  breakpoint definition exists.
- No Figma MCP tool was available in this environment.
- **No Figma design data available.**

## Standard Best Practices (from MCP/skills)

- Libraries should not own presentation concerns; they should expose typed data
  and let the consumer render it.
  Source: `catalogue/31_platform_core.md` ("The public crate never learns a
  product's nouns").
- Accessibility requirements for consuming applications are outside the scope of
  a Rust library crate. They should be defined in the consuming application's own
  constitution.
- No Context7 or project skill files were available in this environment.

## Assumptions

- [to be confirmed] Whether this repository will ever contain a demo UI or a
  documentation site. If it does, this file must be re-evaluated.
- [to be confirmed] The design system, if any, lives in the consuming GIA or
  OmniStream repository, not here.
