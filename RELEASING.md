# Releasing dado

This is a manual runbook. A release is cut from a clean `main` after the feature
branch has been reviewed, tested, and merged. `cargo publish` is irreversible:
crates.io does not allow deleting or replacing a published version.

The example below releases `dado` 0.1.0 with the tag `dado-v0.1.0`.

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
matrix, and the Rust 1.85 MSRV job.

Check that the crate name is still available. A `404` means the name is free:

```sh
curl -s -o /dev/null -w "%{http_code}\n" https://crates.io/api/v1/crates/dado
```

On PowerShell, `curl.exe` avoids the `Invoke-WebRequest` alias. The expected
result is `404`. If the name is taken, stop and choose a fallback before
publishing; do not publish under a different name without approval.

Confirm the version and metadata:

```sh
cd catalogue
cargo metadata --no-deps --format-version 1
```

The `dado` package must report version `0.1.0`, the repository URL, the
`MIT OR Apache-2.0` license, Rust 1.85, and the `yaml`, `diag`, `graph`, `emit`,
and `glob` features.

## 2. Version and changelog

1. Set `version = "0.1.0"` in `catalogue/crates/dado/Cargo.toml`.
2. Add or update the `0.1.0` section in
   `catalogue/crates/dado/CHANGELOG.md`.
3. Confirm `README.md` describes the current API and builds as published.
4. Commit the release preparation:

   ```sh
   git add catalogue/crates/dado/Cargo.toml catalogue/crates/dado/CHANGELOG.md
   git commit -m "specdrive(release): prepare dado 0.1.0"
   git push origin main
   ```

Use the repository's configured branch protection and review process. Do not
bypass required checks.

## 3. Package and dry-run

From `catalogue/`:

```sh
cargo package -p dado --list
cargo package --workspace
cargo publish -p dado --dry-run
```

The package list must include `Cargo.toml`, `README.md`, `CHANGELOG.md`,
`src/**`, `LICENSE-MIT`, and `LICENSE-APACHE`. The dry-run must succeed before
any real publish. Record the result in the checklist below.

## 4. Publish

The token comes from the environment or a CI secret. Never write a token into a
file, commit it, or paste it into an issue.

```sh
export CARGO_REGISTRY_TOKEN="<token supplied by your secret store>"
cd catalogue
cargo publish -p dado
```

PowerShell:

```powershell
$env:CARGO_REGISTRY_TOKEN = "<token supplied by your secret store>"
cd catalogue
cargo publish -p dado
```

`CARGO_REGISTRY_TOKEN` must be a crates.io API token with publish scope for
`dado`. Store it in GitHub Actions secrets only if a workflow publishes; this
runbook performs the publish manually.

## 5. Tag the release

After the publish succeeds, tag the exact commit that was published:

```sh
git tag -a dado-v0.1.0 -m "dado 0.1.0"
git push origin dado-v0.1.0
```

The tag name is fixed: `dado-v0.1.0`. Do not reuse a tag for a different commit.

## 6. Post-publish verification

```sh
cargo search dado --limit 5
cargo info dado
```

In a temporary project:

```sh
cargo new dado-smoke
cd dado-smoke
cargo add dado@0.1.0 --features yaml
```

Build a small program that parses a document with
`dado::yaml::from_str`. The crate must resolve and compile. Confirm the docs.rs
build for `dado` 0.1.0 succeeds with all features enabled.

## 7. Yank or rollback

If a release is broken, yank it rather than trying to replace it:

```sh
cargo yank --version 0.1.0 dado
```

Yanking hides the version from new resolutions but does not delete it or break
existing lockfiles. If the yank was a mistake, undo it:

```sh
cargo yank --version 0.1.0 --undo dado
```

Fix the defect and publish a new patch version, for example `0.1.1`. Never
attempt to overwrite a published version. If the crate name itself is wrong,
stop and document a new release plan before publishing anything else.

## Release checklist

- [ ] `main` is clean and up to date.
- [ ] CI is green on the release commit.
- [ ] `https://crates.io/api/v1/crates/dado` returns `404`.
- [ ] Version is `0.1.0` in `Cargo.toml`.
- [ ] `CHANGELOG.md` has the `0.1.0` entry.
- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo clippy --workspace --all-features -- -D warnings` passes.
- [ ] `cargo test --workspace --all-features` passes.
- [ ] Documentation builds with warnings denied.
- [ ] `cargo package --workspace` succeeds.
- [ ] `cargo publish -p dado --dry-run` succeeds.
- [ ] `CARGO_REGISTRY_TOKEN` is supplied from the environment or a secret store.
- [ ] `cargo publish -p dado` succeeds.
- [ ] Tag `dado-v0.1.0` is pushed.
- [ ] The crate resolves in a fresh project and the YAML example compiles.
- [ ] docs.rs builds with all features.

### Dry-run result

Record the date, command, and outcome here when preparing a release. Do not
publish if this is blank or failing.

```text
Date: 2026-10-20
Command: cargo publish -p dado --dry-run
Result: Passed on a clean `feature/m0-foundation` tree at commit 5aa858f
        (packaged 14 files, 94.0 KiB; upload aborted as expected). Re-run
        from a clean `main` before publishing.
```
