# Releasing

This is a manual runbook. A release is cut from a clean `main` after the feature
branch has been reviewed, tested, and merged. `cargo publish` is irreversible:
crates.io does not allow deleting or replacing a published version.

Each crate has its own section and its own fixed tag name. Publish `dado-yaml` before
`wirekit` when both are being released, because the workspace-level
`cargo package --workspace` and `cargo publish --dry-run` runs cover both.

The first section releases `dado-yaml` 0.1.0 with the tag `dado-yaml-v0.1.0`; the second
releases `wirekit` 0.1.0 with the tag `wirekit-v0.1.0`.

## 1. Pre-flight checks

Run from the repository root unless a step says otherwise.

```sh
git switch main
git pull --ff-only
git status --porcelain
```

`git status --porcelain` must print nothing.

Confirm the release commit is green in GitHub Actions before continuing. The
workflow must have passed format, clippy, docs, package, the three-OS test
matrix, the Rust 1.85 MSRV job, and the dependency audit.

Check that the crate name is still available. A `404` means the name is free:

```sh
curl -s -o /dev/null -w "%{http_code}\n" https://crates.io/api/v1/crates/dado-yaml
```

On PowerShell, `curl.exe` avoids the `Invoke-WebRequest` alias. The expected
result is `404`. If the name is taken, stop and choose a fallback before
publishing; do not publish under a different name without approval.

Confirm the version and metadata:

```sh
cargo metadata --no-deps --format-version 1
```

The `dado-yaml` package must report version `0.1.0`, the repository URL, the
`MIT OR Apache-2.0` license, Rust 1.85, and the `yaml`, `diag`, `graph`, `emit`,
and `glob` features.

## 2. Version and changelog

1. Set `version = "0.1.0"` in `crates/dado-yaml/Cargo.toml`.
2. Add or update the `0.1.0` section in
   `crates/dado-yaml/CHANGELOG.md`.
3. Confirm `crates/dado-yaml/README.md` describes the current API and builds as published.
4. Commit the release preparation:

   ```sh
   git add crates/dado-yaml/Cargo.toml crates/dado-yaml/CHANGELOG.md
   git commit -m "specdrive(release): prepare dado-yaml 0.1.0"
   git push origin main
   ```

Use the repository's configured branch protection and review process. Do not
bypass required checks.

## 3. Package and dry-run

From the repository root:

```sh
cargo package -p dado-yaml --list
cargo package --workspace
cargo publish -p dado-yaml --dry-run
```

The package list must include `Cargo.toml`, `README.md`, `CHANGELOG.md`,
`src/**`, `LICENSE-MIT`, and `LICENSE-APACHE`. The dry-run must succeed before
any real publish. Record the result in the checklist below.

## 4. Publish

The token comes from the environment or a CI secret. Never write a token into a
file, commit it, or paste it into an issue.

```sh
export CARGO_REGISTRY_TOKEN="<token supplied by your secret store>"
cargo publish -p dado-yaml
```

PowerShell:

```powershell
$env:CARGO_REGISTRY_TOKEN = "<token supplied by your secret store>"
cargo publish -p dado-yaml
```

`CARGO_REGISTRY_TOKEN` must be a crates.io API token with publish scope for
`dado-yaml`. Store it in GitHub Actions secrets only if a workflow publishes; this
runbook performs the publish manually.

## 5. Tag the release

After the publish succeeds, tag the exact commit that was published:

```sh
git tag -a dado-yaml-v0.1.0 -m "dado-yaml 0.1.0"
git push origin dado-yaml-v0.1.0
```

The tag name is fixed: `dado-yaml-v0.1.0`. Do not reuse a tag for a different commit.

## 6. Post-publish verification

```sh
cargo search dado-yaml --limit 5
cargo info dado-yaml
```

In a temporary project:

```sh
cargo new dado-yaml-smoke
cd dado-yaml-smoke
cargo add dado-yaml@0.1.0 --features yaml
```

Build a small program that parses a document with
`dado_yaml::yaml::from_str`. The crate must resolve and compile. Confirm the docs.rs
build for `dado-yaml` 0.1.0 succeeds with all features enabled.

## 7. Yank or rollback

If a release is broken, yank it rather than trying to replace it:

```sh
cargo yank --version 0.1.0 dado-yaml
```

Yanking hides the version from new resolutions but does not delete it or break
existing lockfiles. If the yank was a mistake, undo it:

```sh
cargo yank --version 0.1.0 --undo dado-yaml
```

Fix the defect and publish a new patch version, for example `0.1.1`. Never
attempt to overwrite a published version. If the crate name itself is wrong,
stop and document a new release plan before publishing anything else.

## dado-yaml release checklist

- [ ] `main` is clean and up to date.
- [ ] CI is green on the release commit.
- [ ] `https://crates.io/api/v1/crates/dado-yaml` returns `404`.
- [ ] Version is `0.1.0` in `crates/dado-yaml/Cargo.toml`.
- [ ] `crates/dado-yaml/CHANGELOG.md` has the `0.1.0` entry.
- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo clippy --workspace --all-features -- -D warnings` passes.
- [ ] `cargo test --workspace --all-features` passes.
- [ ] Documentation builds with warnings denied.
- [ ] `cargo package --workspace` succeeds.
- [ ] `cargo publish -p dado-yaml --dry-run` succeeds.
- [ ] `CARGO_REGISTRY_TOKEN` is supplied from the environment or a secret store.
- [ ] `cargo publish -p dado-yaml` succeeds.
- [ ] Tag `dado-yaml-v0.1.0` is pushed.
- [ ] The crate resolves in a fresh project and the YAML example compiles.
- [ ] docs.rs builds with all features.

### Dry-run result

Record the date, command, and outcome here when preparing a release. Do not
publish if this is blank or failing.

```text
Date: 2026-10-20
Command: cargo publish -p dado-yaml --dry-run
Result: Passed on a clean `feature/m0-foundation` tree (packaged 14 files,
        94.0 KiB; upload aborted as expected). Re-run from a clean `main`
        before publishing.
```

# Releasing wirekit

`wirekit` 0.1.0 ships the core plus four provider modules. The release is cut
from the same clean `main` as `dado-yaml`, after the M1 feature branch has been
reviewed, tested, and merged. The tag is `wirekit-v0.1.0`.

## 1. Pre-flight checks

```sh
git switch main
git pull --ff-only
git status --porcelain
```

`git status --porcelain` must print nothing, and CI must be green on the release
commit for format, clippy, docs, package, the three-OS test matrix, and the Rust
1.85 MSRV job.

Confirm the name is still free. A `404` means it is:

```sh
curl -s -o /dev/null -w "%{http_code}\n" https://crates.io/api/v1/crates/wirekit
```

On PowerShell, use `curl.exe` to avoid the `Invoke-WebRequest` alias.

Confirm the metadata:

```sh
cargo metadata --no-deps --format-version 1
```

The `wirekit` package must report version `0.1.0`, the repository URL, the
`MIT OR Apache-2.0` license, Rust 1.85, and the `noaa`, `twilio`, `sendgrid`,
`slack`, and `native-tls` features.

## 2. Version and changelog

1. Set `version = "0.1.0"` in `crates/wirekit/Cargo.toml`.
2. Confirm the `0.1.0` section in `crates/wirekit/CHANGELOG.md` lists every
   provider module that ships.
3. Confirm `crates/wirekit/README.md` matches the published feature set and the
   `W2001`–`W2007` error table.
4. Commit the release preparation:

   ```sh
   git add crates/wirekit/Cargo.toml crates/wirekit/CHANGELOG.md
   git commit -m "specdrive(release): prepare wirekit 0.1.0"
   git push origin main
   ```

## 3. Package and dry-run

The core compiles with no provider feature, and each provider compiles alone.
Check the matrix before packaging:

```sh
cargo check -p wirekit
cargo check -p wirekit --no-default-features --features noaa
cargo check -p wirekit --no-default-features --features twilio
cargo check -p wirekit --no-default-features --features sendgrid
cargo check -p wirekit --no-default-features --features slack
cargo check -p wirekit --all-features
```

Then package and dry-run:

```sh
cargo package -p wirekit --list
cargo package --workspace
cargo publish -p wirekit --dry-run
```

The package list must include `Cargo.toml`, `README.md`, `CHANGELOG.md`,
`src/**`, `examples/**`, `LICENSE-MIT`, and `LICENSE-APACHE`. The dry-run must
succeed before any real publish. `cargo publish` never runs in CI or in the
implementation phase; it is a manual step.

## 4. Publish

```sh
export CARGO_REGISTRY_TOKEN="<token supplied by your secret store>"
cargo publish -p wirekit
```

PowerShell:

```powershell
$env:CARGO_REGISTRY_TOKEN = "<token supplied by your secret store>"
cargo publish -p wirekit
```

The token must have publish scope for `wirekit`. Never write it into a file,
commit it, or paste it into an issue.

## 5. Tag the release

```sh
git tag -a wirekit-v0.1.0 -m "wirekit 0.1.0"
git push origin wirekit-v0.1.0
```

The tag name is fixed: `wirekit-v0.1.0`. Do not reuse it for a different commit.

## 6. Post-publish verification

```sh
cargo search wirekit --limit 5
cargo info wirekit
```

In a temporary project:

```sh
cargo new wirekit-smoke
cd wirekit-smoke
cargo add wirekit@0.1.0 --features noaa
```

Fetch a forecast with `NoaaClient` as the README shows. Then repeat with
`--features twilio`, `sendgrid`, and `slack` to confirm each feature resolves
from crates.io. Confirm the docs.rs build succeeds with all features enabled.

## wirekit release checklist

- [ ] `main` is clean and up to date.
- [ ] CI is green on the release commit.
- [ ] `https://crates.io/api/v1/crates/wirekit` returns `404`.
- [ ] Version is `0.1.0` in `crates/wirekit/Cargo.toml`.
- [ ] `crates/wirekit/CHANGELOG.md` has the `0.1.0` entry.
- [ ] The feature matrix checks from section 3 pass.
- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo clippy --workspace --all-features -- -D warnings` passes.
- [ ] `cargo test --workspace --all-features` passes.
- [ ] Documentation builds with warnings denied.
- [ ] `cargo package --workspace` succeeds.
- [ ] `cargo publish -p wirekit --dry-run` succeeds.
- [ ] `CARGO_REGISTRY_TOKEN` is supplied from the environment or a secret store.
- [ ] `cargo publish -p wirekit` succeeds.
- [ ] Tag `wirekit-v0.1.0` is pushed.
- [ ] Every provider feature resolves in a fresh project.
- [ ] docs.rs builds with all features.

### wirekit dry-run result

Record the date, command, and outcome here when preparing a release. Do not
publish if this is blank or failing.

```text
Date: 2026-10-21
Command: cargo publish -p wirekit --dry-run
Result: Pending. Run from a clean `main` after the M1 branch is merged; the
        M1 implementation phase records `cargo package -p wirekit --list` only.
```

