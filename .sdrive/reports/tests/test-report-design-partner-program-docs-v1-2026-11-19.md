# Test Report: design-partner-program-docs (v1)

- **Date:** 2026-11-19
- **Commit:** `ee3d6546c13df3f0439ac337995f888f69839b2d`
- **Branch:** `feature/w4`
- **Command:** `node scripts/rust-tests.js` (→ `cargo test --workspace --all-features`)
- **Result:** **PASS** — exit 0, 275 passed, 0 failed, 2 skipped, 277 total
- **Evidence:** `.sdrive/governance/design-partner-program-docs/v1/test-evidence.json`

## 1. Summary

The declared project test command now passes. It previously exited 101 for every
feature in the workspace, because `ring v0.17.14`'s build script could not compile
`crypto/curve25519/curve25519.c` under MSVC 14.50 (VS 18 Build Tools) with
`D8050: cannot execute c1.dll`.

**That diagnosis was wrong in the 2026-11-17 review.** `c1.dll` is present and
healthy in the toolset at
`…\BuildTools\VC\Tools\MSVC\14.50.35717\bin\HostX64\x64\`. Nothing needed
repairing. The real fault is that MSVC 14.50 rejects the `/Z7` debug records
`cc-rs` passes to `cl.exe`, and `cc-rs` only adds `/Z7` when the cargo profile has
debug info on. The release profile therefore always worked.

**Fix applied:** four lines in the workspace `Cargo.toml`:

```toml
[profile.dev.package.ring]
debug = false
```

This disables debug info for `ring` alone. The dev profile stays intact for every
other crate, no dependency changed (`Cargo.lock` is byte-identical), and the
declared test command is unchanged. It should be removed once `ring`/`cc-rs` is
bumped to a version that handles MSVC 14.50.

## 2. Acceptance Criteria Coverage

All six ACs have a named test, and each was proven red-green by mutating the
document and confirming the shipped checker fails.

| AC | Test | Status | Red-green proof |
|----|------|--------|-----------------|
| AC-001 | `ac_001_program_states_eligibility_benefits_obligations_and_production` | pass | Softened both "missing evidence blocks 1.0" statements → checker exit 1 |
| AC-002 | `ac_002_tracker_is_empty_seven_column_template_without_invented_partner` | pass | Inserted a real partner row with a live evidence link → checker exit 1 |
| AC-003 | `ac_003_case_study_has_consent_redaction_and_measured_outcomes` | pass | Removed the credentials checklist line → exit 1; weakened "written approval" → exit 1 |
| AC-004 | `ac_004_readme_links_repository_root_to_program_page` | pass | Unlinked the program page from README → checker exit 1 |
| AC-005 | `ac_005_claims_are_sourced_or_marked_placeholder` | pass | Invented an unsourced partner row → checker exit 1 |
| AC-006 | `ac_006_program_names_g5_and_blocks_1_0_without_evidence` | pass | Replaced the 30-day production test with "a reasonable period" → exit 1 |

**Coverage: 6 of 6 ACs (100%).**

### Note on the mutation method

The tests do not re-implement the document rules; they run the shipped checker
(`scripts/docs-partners.js`) and assert on its exit status. That is deliberate —
a second implementation would agree with the checker only until one of them was
edited. The consequence is that the checker's own rules are the thing under test,
so the six mutations above are the real proof that the suite has teeth.

One mutation initially **passed** and is worth recording: softening only one of the
two sentences in `program.md` that state the 1.0 gate. The document states the rule
twice (lines 33-36), so a single edit is caught by the other copy. Softening both is
caught by the checker. That is document redundancy, not a gap in the test.

## 3. Test Quality

- Six tests, one per AC, each carrying its AC ID in the name (CON-405).
- No `.only`, `xit`, `#[ignore]`, `todo!`, or `unimplemented!` in the feature's tests.
- Tests live at `crates/design-partner-program-docs/tests/docs_partners/<subfeature>.rs`,
  satisfying CON-406 via explicit `[[test]]` path declarations in `Cargo.toml`.
- The crate declares no dependencies and exposes no API; it exists only for these tests.
- 2 tests are skipped workspace-wide, both from other crates and both explicitly
  gated on external state (`ac_029` needs a published crate, `ac_030` needs a
  release tag). Neither belongs to this feature.

## 4. Verification Checks

| Check | Command | Result |
|-------|---------|--------|
| Declared test command | `node scripts/rust-tests.js` | **pass**, exit 0, 275/0/2 |
| Workspace suite | `cargo test --workspace --all-features` | **pass**, exit 0 |
| Docs checker | `node scripts/docs-partners.js` | **pass**, exit 0 |
| Governance validation | `node .sdrive/scripts/validate-governance.js` | **pass**, exit 0, all files valid |
| Traceability | 11 rows | all `complete`, all `testIds` populated |
| Tasks | 5 of 5 | T1.1-T1.3, T2.1, T2.2 all complete |

## 5. Limitations

- **CI not verified.** No CI run exists for `ee3d654`. The suite was verified
  locally on Windows only.
- **MSVC workaround is a local-environment fix.** The `[profile.dev.package.ring]`
  override is committed to the workspace, so it applies to every developer and to
  CI. CI on Linux/macOS is unaffected (it does not use MSVC), and CI on Windows
  benefits from it. It is a workaround, not a repair of the underlying
  `ring`/`cc-rs` incompatibility.
- **Release profile was also verified** (`cargo test --workspace --all-features
  --release`, exit 0) to confirm the failure was profile-specific and not a
  genuine workspace breakage.
- **`sdrive` CLI is not installed** on this machine, so evidence was recorded
  directly against `.sdrive/schemas/test-evidence.schema.json` rather than through
  `sdrive test`. The recorded counts come from a real run of the declared command
  at the recorded commit.
