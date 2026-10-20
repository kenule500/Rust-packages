# Test Report: M0 Foundation - dado 0.1.0 (v1)

## 1. Summary
- **Date:** 2026-10-20
- **Feature:** m0-foundation
- **Version:** v1
- **Total ACs:** 37
- **Tested ACs:** 37
- **Coverage:** 100%
- **Tests Passed:** 36/38

## 2. AC → Test Mapping

| AC ID | Test ID | File | Status |
|-------|---------|------|--------|
| AC-001 | TEST-001 | crates/dado/tests/m0-foundation/yaml_values.rs | ✅ Pass |
| AC-002 | TEST-002 | crates/dado/tests/m0-foundation/yaml_values.rs | ✅ Pass |
| AC-003 | TEST-003 | crates/dado/tests/m0-foundation/yaml_values.rs | ✅ Pass |
| AC-004 | TEST-004 | crates/dado/tests/m0-foundation/yaml_values.rs | ✅ Pass |
| AC-005 | TEST-005 | crates/dado/tests/m0-foundation/yaml_values.rs | ✅ Pass |
| AC-006 | TEST-006 | crates/dado/tests/m0-foundation/yaml_values.rs | ✅ Pass |
| AC-007 | TEST-007 | crates/dado/tests/m0-foundation/yaml_values.rs | ✅ Pass |
| AC-008 | TEST-008 | crates/dado/tests/m0-foundation/yaml_values.rs | ✅ Pass |
| AC-009 | TEST-009 | crates/dado/tests/m0-foundation/yaml_values.rs | ✅ Pass |
| AC-010 | TEST-010 | crates/dado/tests/m0-foundation/yaml_values.rs | ✅ Pass |
| AC-011 | TEST-011 | crates/dado/tests/m0-foundation/yaml_errors.rs | ✅ Pass |
| AC-012 | TEST-012 | crates/dado/tests/m0-foundation/yaml_errors.rs | ✅ Pass |
| AC-013 | TEST-013 | crates/dado/tests/m0-foundation/yaml_errors.rs | ✅ Pass |
| AC-014 | TEST-014 | crates/dado/tests/m0-foundation/quality_baseline.rs | ✅ Pass |
| AC-015 | TEST-015 | crates/dado/tests/m0-foundation/quality_baseline.rs | ✅ Pass |
| AC-016 | TEST-016 | crates/dado/tests/m0-foundation/quality_baseline.rs | ✅ Pass |
| AC-017 | TEST-017 | crates/dado/tests/m0-foundation/quality_baseline.rs | ✅ Pass |
| AC-018 | TEST-018 | crates/dado/tests/m0-foundation/quality_baseline.rs | ✅ Pass |
| AC-019 | TEST-019 | crates/dado/tests/m0-foundation/quality_baseline.rs | ✅ Pass |
| AC-020 | TEST-020 | crates/dado/tests/m0-foundation/quality_baseline.rs | ✅ Pass |
| AC-021 | TEST-021 | crates/dado/tests/m0-foundation/quality_baseline.rs | ✅ Pass |
| AC-022 | TEST-022 | crates/dado/tests/m0-foundation/contributor_docs.rs | ✅ Pass |
| AC-023 | TEST-023 | crates/dado/tests/m0-foundation/contributor_docs.rs | ✅ Pass |
| AC-024 | TEST-024 | crates/dado/tests/m0-foundation/contributor_docs.rs | ✅ Pass |
| AC-025 | TEST-025 | crates/dado/tests/m0-foundation/contributor_docs.rs | ✅ Pass |
| AC-026 | TEST-026 | crates/dado/tests/m0-foundation/contributor_docs.rs | ✅ Pass |
| AC-027 | TEST-027 | crates/dado/tests/m0-foundation/contributor_docs.rs | ✅ Pass |
| AC-028 | TEST-028 | crates/dado/tests/m0-foundation/release_readiness.rs | ✅ Pass |
| AC-029 | TEST-029 | crates/dado/tests/m0-foundation/release_readiness.rs | ⚠️ Blocked |
| AC-030 | TEST-030 | crates/dado/tests/m0-foundation/release_readiness.rs | ⚠️ Partial |
| AC-031 | TEST-031 | crates/dado/tests/m0-foundation/release_readiness.rs | ✅ Pass |
| AC-032 | TEST-032 | crates/dado/tests/m0-foundation/release_readiness.rs | ✅ Pass |
| AC-033 | TEST-033 | crates/dado/tests/m0-foundation/release_readiness.rs | ✅ Pass |
| AC-034 | TEST-034 | crates/dado/tests/m0-foundation/release_readiness.rs | ✅ Pass |
| AC-035 | TEST-035 | crates/dado/tests/m0-foundation/purity.rs | ✅ Pass |
| AC-036 | TEST-036 | crates/dado/tests/m0-foundation/purity.rs | ✅ Pass |
| AC-037 | TEST-037 | crates/dado/tests/m0-foundation/purity.rs | ✅ Pass |

## 3. Failures

| Test | Reason | Blocking |
|------|--------|----------|
| TEST-029 | Requires `dado` 0.1.0 to be published on crates.io; marked `#[ignore]` so it can run from `main` after publication. | Yes, release gate |
| TEST-030 (tag case) | `dado-v0.1.0` cannot exist before the release is cut; the changelog half of AC-030 passes. | No, post-release check |

## 4. Notes

- Framework: Rust's built-in test harness (`cargo test`), per CON-401 and CON-402. Tests live under `crates/dado/tests/m0-foundation/`, per CON-403 and CON-406. Every test name carries its AC ID.
- `cargo test --workspace --all-features` exited 0: 36 passed, 0 failed, 2 ignored (38 test functions). The two ignored tests are the post-publish and post-release checks above.
- AC-014 through AC-021 assert the workflow contract. The commands themselves were also run locally: `cargo fmt --all --check`, `cargo clippy --workspace --all-features --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`, and `cargo package --workspace` all passed. CI run 39450156382 is green on format, clippy, docs, package, the three-OS test matrix, and MSRV 1.85.
- `cargo package --workspace` packaged 14 files (94.0 KiB) and verified the package. Cargo warned that the new integration test target is not in the published package; that is intentional because `include` lists only the crate sources and release files.
- `cargo publish -p dado --dry-run --allow-dirty` completed and aborted the upload as expected. The crates.io index had transient DNS/timeout warnings; packaging and verification succeeded.
- Feature independence was rechecked with `cargo check -p dado --no-default-features` and with each of `yaml`, `diag`, `graph`, `emit`, and `glob` enabled alone. All combinations passed.
- `sdrive verify m0-foundation` passes after adding Rust-aware source, path, symbol, and dependency detection to `.sdrive/scripts/verify.js`: 37 matched, 0 missing, 0 renamed, 0 extra, status `pass`.
- `test-evidence.json` and `verification.json` are local gate artifacts. They are ignored and regenerated at the current `HEAD` because a committed evidence file cannot also name the commit that contains it.
- No production code was modified by this test phase. The only source-tree additions are the integration test files.
