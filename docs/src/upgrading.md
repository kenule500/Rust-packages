# Upgrading

Every crate in this workspace is `0.1.0`. Nothing is published to crates.io yet,
so every consumer today builds from the repository.

## What `0.x` promises

`0.x` is explicitly unstable in semver terms, and this workspace does not pretend
otherwise:

- A **patch** release (`0.1.0` to `0.1.1`) fixes defects and adds nothing that
  breaks existing code.
- A **minor** release (`0.1` to `0.2`) may break the API. That is what the major
  position means before `1.0`.
- **Stable error codes are the exception to every version rule.** A published
  code such as `W2004` or `QL-EXCEEDED` keeps its meaning for the life of the
  crate; a new failure gets a new code (CON-205).

Because a minor release may break, the upgrade path for `0.x` is the changelog,
not a deprecation window. Read the changelog entry for the version you are moving
to.

## What `1.0` will promise

`1.0` is a decision about evidence, not a date (see the 1.0 gate in the
repository README): three external projects using the crates in production, and
GIA in production. When `1.0` is cut, semver takes over in full: a breaking change
waits for a major version, and every break is recorded before it ships.

## Where breaking changes are recorded

| Record | What it covers | Location |
|---|---|---|
| `CHANGELOG.md` | Per-crate user-visible changes, including every break | `crates/<crate>/CHANGELOG.md` |
| Release runbook | The mechanical steps, tags, and pre-flight checks for a release | [`RELEASING.md`](https://github.com/kenule500/Rust-packages/blob/main/RELEASING.md) |
| Catalogue design note | The intended surface of a provider module | [`catalogue/`](https://github.com/kenule500/Rust-packages/tree/main/catalogue) |

Not every crate has a `CHANGELOG.md` yet: it is created with the first release of
that crate, in the release runbook's "Version and changelog" step. A crate that
has never been released has no published API to break.

## Upgrading a provider dependency

`wirekit` provider modules are additive Cargo features: enabling one never
enables another (CON-204), and the default feature set enables none of them. An
upgrade never has to reason about a provider you did not ask for.

```toml
wirekit = { version = "0.1", features = ["twilio", "sendgrid"] }
```

If a provider feature disappears, the change is a break and is listed in the
`wirekit` changelog.

## Checklist for an upgrade

1. Read the changelog entry for the version you are moving to, and the entries
   for every version in between. A `0.x` minor may break in several places at
   once.
2. Build with the same feature set you shipped. A provider you did not enable
   cannot break you; a provider you did enable is the one to test.
3. Search your code for the stable error codes you match on. Those do not
   change, so a match arm that stops compiling means the code was renamed by
   mistake — report it rather than working around it.
4. Run your own test suite. This repository's suite proves the crate, not your
   call sites.
