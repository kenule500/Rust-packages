# Design partner program

> **Status:** Approved program, no partners recruited yet
> **Date:** 2026-11-08
> **Decision gate:** G5 and G6 in `.sdrive/product/overview.md`; 1.0 is in `.sdrive/product/roadmap.md`

This page defines who counts as a design partner, what both sides get, what both
sides owe, how "in production" is judged, and what the program is for. It is the
record the 1.0 decision is checked against, so it is written to make a weak claim
visible rather than to make the program sound impressive.

The program currently has no partners, and every row in
[`tracker.md`](tracker.md) is a placeholder.

## Why the program exists

Adoption is the item the roadmap names as the hardest and the one most likely to
be deferred to the end. A package can be complete, tested, and reviewed and still
be wrong about the things only a real consumer notices: the shape of the error you
get at 3 a.m., the field the API docs omit, the retry behaviour that fights your
scheduler. The program exists to get that feedback while the API is still cheap to
change, and to leave a record that 1.0 is owed to real use rather than to
enthusiasm.

## The 1.0 evidence gate

1.0 is a decision about evidence, not a date. Two gates from
`.sdrive/product/overview.md` and `.sdrive/product/roadmap.md` apply:

- **G5**: three external projects use `wirekit` in production.
- **G6**: GIA uses `wirekit` in production.

The 1.0 freeze happens only after both. If the evidence is missing, 1.0 does not
happen. The version is not softened to "1.0-rc" to keep a schedule, and a published
0.x is not relabelled as 1.0 in a README. Missing evidence blocks the gate; that is
the whole point of having it.

GIA is one internal consumer, so G6 is a real but weak check. Three external teams
(G5) is what proves the API survives people who are not us.

## Eligibility

A project qualifies when all of these hold:

1. **Independent.** It is not maintained by the author of these crates, and it is
   not a test fixture written to satisfy the gate.
2. **In production.** Live, with real traffic from real users. See below for what
   production means.
3. **On a released version.** It consumes the crates from crates.io, not a path or
   git dependency on an unpublished branch. A partner cannot validate software
   nobody can install.
4. **Consenting.** A named contact has agreed in writing to the program's
   obligations. Being privately listed, publicly listed, and named in a case study
   are three separate yeses.

Access needs are not eligibility. A team that cannot get Adobe, ATTOM, or CoreLogic
credentials is a provider decision, reconsidered in the roadmap, not a partner
question.

## What a partner receives

- Reproduction of a provider decision, such as a provider promoted ahead of the
  remaining-provider batch in the roadmap milestone.
- Patch releases on the rare bug that blocks their production path, not just a place
  in a queue.
- A monthly written status: what shipped, what is next, and what is blocking.
- An invitation to 1.0 discussions, with a one-release notice of breaking changes
  for their published path. The 0.x instability recorded in the workspace README
  still applies to the rest of the API.

## What a partner commits to

- **A real production integration.** Live, with real traffic. A development-only
  integration with no plan to ship, and a demo, both fail eligibility.
- **A named contact role**: a maintainer, an engineer, or a lead. The row records the
  role, never the private individual, and a consent refers to the role, never to a
  person's data.
- **Permission to be listed**, one of: private and not listed, listed, or listed and
  named in a case study.
- **One call or written report per month** for the first quarter: what works, what is
  awkward, and what is missing.
- **Feedback access.** Release candidates and breaking-change announcements are sent
  only to partners who have consented, and a removal is honored on request.

## Obligations of the project

- **No silent breaks.** A breaking change gets a deprecation, a migration note, and a
  partner notice one release ahead of removal.
- **Partner-visible decisions.** Provider and design decisions that affect a partner
  are explained, not just shipped.
- **Bug response.** A production-blocking partner bug has priority within five working
  days, even without a fix yet, through the issue or the partner's channel.
- **Credits and truth.** The program claims only what is recorded: consent on file,
  a production window, and a published case study when approved.
- **Removal is honoured.** A partner can leave the program or ask to be delisted at any
  time, no questions asked; the tracker row is deleted, and any public case study is
  unpublished.

## What "in production" means

Production (AC-001) has one test, and it is checkable:

- at least one crate is used from crates.io, with a non-prerelease version.
- a live deployment receives real traffic from real users or real customers.
  Internal tools count when people outside the maintainer's team use them daily.
- the deployment has been live for at least 30 consecutive days by the date of
  the check.
- the deployment owner can confirm the above in writing.

Not production: a dev or demo environment; samples; CI use; a prototype nobody
depends on; runtime use only inside a test harness. Where the date is disputed,
the 30-day window starts at the first deploy the owner can point at, not at when
someone first tried the crate.

## Program forms

- **Private partner.** Consent on file, no public listing. Counts toward G5.
- **Listed partner.** Consent on file plus permission to list the project in the
  tracker. Counts toward G5. The name appears only after the consent row exists.
- **Case-study partner.** Listed, with written approval to publish outcomes. The
  case study goes through
  [`case-study-template.md`](case-study-template.md), including its consent
  statement and redaction checklist.

A listing is not permission to write a case study. Only the written approval is.