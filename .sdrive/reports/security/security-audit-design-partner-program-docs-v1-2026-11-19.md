# Security Audit Report: Design partner program and case-study docs (v1)

## 1. Executive Summary

- **Date:** 2026-11-19
- **Auditor:** AI Security Agent (`/sdrive:secure`)
- **Feature Version:** v1
- **Branch / base:** `feature/w4` / `main`
- **Commit audited:** `ee3d6546c13df3f0439ac337995f888f69839b2d`
- **Overall Risk Level:** Low
- **Total Vulnerabilities Found:** 0 (Critical: 0, High: 0, Med: 0, Low: 0)
- **Governance defects found (non-security):** 2 (Low)

The feature is a documentation deliverable: three Markdown pages under
`docs/design-partners/`, a zero-dependency Node checker (`scripts/docs-partners.js`),
and a test-only crate (`crates/design-partner-program-docs`) whose six acceptance
tests exercise the shipped checker. There is no runtime, no network surface, no
database, no authentication, and no user input path. The security surface is
therefore narrow, and it is clean.

This audit **confirms** the `pass` security verdict recorded by `/sdrive:review` on
2026-11-17 and extends it with a mutation test of the checker itself, which the
review did not perform.

## 2. Threat Model Summary

- **Key Assets:**
  - Partner identity and consent records (personal data, held outside the repo).
  - The integrity of the 1.0 evidence gate: a fabricated partner row would falsely
    certify adoption and corrupt the release decision.
  - The repository's public surface: these pages render on GitHub to prospective
    partners.
- **Trust Boundaries:**
  - Repository content → GitHub rendering (untrusted Markdown is rendered by a third
    party; raw HTML would cross into an executable context).
  - Future contributor → tracker table (a row is a claim; an unsourced row is an
    unverified assertion entering a decision record).
  - Local checker process → filesystem (the only I/O the feature performs).
- **Actors:** a contributor adding a partner row; a partner exercising removal; a
  reader of the published pages. No remote or unauthenticated attacker has a path
  into this feature, because nothing here accepts input at runtime.

## 3. Vulnerability Details

No vulnerabilities were identified. The controls that were verified:

### CON-519 Environment Security — PASS
- No credentials, API keys, tokens, or private contact details appear in any of the
  three documents, the checker, or the tests.
- Verified by pattern scan over `docs/design-partners/*.md`,
  `scripts/docs-partners.js`, and `crates/design-partner-program-docs/tests/docs_partners/*.rs`
  for `api_key|secret|token|password|bearer|sk-|ghp_|AKIA|BEGIN *PRIVATE KEY`.
  **One match, and it is the rule itself**: `case-study-template.md:83` reads
  "No credentials, API keys, tokens, webhook URLs, or private endpoints." That is a
  redaction instruction, not a value.
- No email addresses, phone numbers, or personal names in the documents. The
  tracker records a contact **role** by design (`tracker.md:13-14`), and the
  case-study template attributes quotes by role only (`case-study-template.md:87`).
- The feature diff against `main` adds no high-entropy string (no match for 40+
  character base64/hex-like runs in added lines).

### CON-520 HTTPS / HSTS — PASS
- No `http://` target anywhere in `docs/design-partners/*.md` or in the README
  section the feature added.
- The documents contain **no external links at all**. Every link is a relative
  repository path, and all five resolve:
  - `program.md → tracker.md`, `program.md → case-study-template.md`
  - `tracker.md → program.md`
  - `case-study-template.md → program.md`, `case-study-template.md → tracker.md`
  - `README.md:154,156 → docs/design-partners/program.md`, `…/tracker.md`
- The two `.sdrive/product/` paths cited in `program.md:5` and `:28` exist on disk.
  They are prose references, not Markdown links, so they do not render as clickable
  targets; noted for accuracy, not as a finding.

### CON-517 XSS — NOT APPLICABLE (confirmed)
- No `<script>`, `<iframe>`, `<img>`, `<a >`, `<style>`, `javascript:`, `onerror=`,
  `onload=`, or `data:`/`file:`/`vbscript:` URI in any document. The pages are plain
  Markdown with tables and blockquotes, and GitHub's renderer sanitises what it does
  emit.

### CON-511, CON-512, CON-513, CON-514, CON-515, CON-516, CON-518 — NOT APPLICABLE
- Confirmed against the code, not assumed. There is no authentication, no tenant
  data, no HTTP server, no rate limiter, no runtime input parser, no database, and
  no form or cookie-based state. `crates/design-partner-program-docs/src/lib.rs` is
  two comment lines and exposes no API; `Cargo.toml` declares **no dependencies**.

### Checker attack surface — PASS
- `scripts/docs-partners.js` requires exactly two modules: `fs` and `path`
  (lines 12-13). No `child_process`, no `exec`/`spawn`, no `fetch`/`http`, no
  `eval`, no `new Function`. It reads three known files plus `README.md` and writes
  nothing. It cannot be steered by document content into executing anything.
- Its only failure mode is `process.exit(1)` with a printed reason.

### Dependency exposure — PASS
- `Cargo.lock` is **unchanged** by this feature (`git diff main...HEAD -- Cargo.lock`
  is empty). The new crate has an empty `[dependencies]` table.
- `cargo audit` (0.22.2, available on this machine) reports **one allowed warning**
  and no vulnerability: `chrono 0.4.45`, `RUSTSEC-2026-0543`, "chrono is
  unmaintained", dated 2026-09-14. This is a workspace-wide advisory inherited from
  existing crates, **not introduced by this feature**, and the audit treats it as
  allowed. The run also emitted registry-timeout errors while checking yanked
  status, so the yanked-package check is **incomplete** — see Limitations.

## 4. Security Regression Verification

- [x] **Secret scan passed** — pattern-based, over the feature files and the added
      diff lines. `gitleaks` and `trufflehog` are **not installed** on this machine,
      so this is a manual scan, not a tool-verified one.
- [x] **Dependency audit run** — `cargo audit` completed with no vulnerabilities and
      one allowed unmaintained-crate warning (`chrono`, pre-existing).
- [x] **Acceptance tests passed** — `cargo test -p design-partner-program-docs`
      exits **0**: 6 tests, 6 passed, 0 failed, 0 ignored
      (`ac_001`, `ac_002`, `ac_003`, `ac_004`, `ac_005`, `ac_006`).
- [x] **Checker passed** — `node scripts/docs-partners.js` exits **0**.
- [x] **Governance validation passed** — `node .sdrive/scripts/validate-governance.js`
      exits **0**, "All files are valid."
- [x] **Mutation test of the checker** — performed by this audit. The checker was
      confirmed to *fail* (exit 1) on each of five regressions, and the working tree
      was restored to clean afterwards (`git status --porcelain` empty):

      | # | Mutation | Result |
      |---|----------|--------|
      | M1 | Removed the credentials line from the redaction checklist | exit 1 — "redaction checklist must cover credentials" |
      | M2 | Weakened "written approval" to "approval" in the consent statement | exit 1 — "consent must require written approval" |
      | M3 | Invented a partner row with a real evidence link | exit 1 — "row 20/21/22 names a partner without a placeholder evidence link" |
      | M4 | Softened **both** statements that missing evidence blocks 1.0 | exit 1 — "must state that missing evidence blocks 1.0" |
      | M5 | Replaced the 30-consecutive-day production test with "a reasonable period" | exit 1 — "must define in production as live for at least 30 consecutive days" |

      M4 is worth recording: a first attempt that softened only one of the two gate
      sentences **passed**, because `program.md` states the rule twice (lines 33-36).
      That is redundancy in the document, not a hole in the checker — softening both
      is caught. The checker's regexes are anchored to meaning, not to one phrasing.

- **Limitations:**
  - `gitleaks` / `trufflehog` absent → secret detection is pattern-based and may miss
    an unusual credential format. No credential-shaped value exists in these files,
    so the residual risk is negligible.
  - `cargo audit` could not complete its yanked-package check (registry timeouts).
    Vulnerability advisories were still evaluated; only yank status is unverified.
  - The declared project test command `node scripts/rust-tests.js` (→
    `cargo test --workspace --all-features`) exited 101 on this machine at the time
    of the audit, because MSVC 14.50 rejects the `/Z7` debug records that
    `ring 0.17.14`'s build script passes to `cl.exe` (`D8050: cannot execute
    c1.dll`). That is a **local toolchain defect, not a security finding and not
    caused by this feature**. It has since been fixed in the workspace `Cargo.toml`
    with `[profile.dev.package.ring] debug = false`; the declared command now exits
    0 with 275 passed. See
    `.sdrive/reports/tests/test-report-design-partner-program-docs-v1-2026-11-19.md`.
    **Correction:** this report initially repeated the 2026-11-17 review's advice to
    "repair the MSVC v143 x64 component". That advice was wrong — `c1.dll` is
    present and healthy in the toolset. No repair was needed.
  - No CI run exists for the audited commit; CI was not verified.

## 5. Recommendations

1. **Retire the `ring` debug workaround once `ring`/`cc-rs` is bumped.** The
   workspace now carries `[profile.dev.package.ring] debug = false` so the dev
   profile can compile `ring` under MSVC 14.50. That is a workaround for an upstream
   incompatibility, not a repair, and it should be removed when the dependency is
   updated. Not a security matter, but it should not be forgotten.

2. **Install `gitleaks`** and wire it into CI. Pattern scanning is a fallback, and
   this project handles provider credentials across many crates; the scanner should
   not depend on an agent remembering to grep.
3. **Keep the checker's mutation coverage.** The five mutations above are the
   regression suite for the documents' honesty rules. If the checker is ever
   rewritten, re-run them — a checker that passes on a fabricated partner row is
   worse than no checker, because it certifies the gate.
4. **Fix the two Low governance defects** found in passing (below). Neither is a
   security issue; both are cheap.

## 6. Non-Security Defects Noted In Passing

| Severity | Defect | Location |
|----------|--------|----------|
| Low | Typo `crates.ioc` where `crates.io` is meant | `docs/design-partners/tracker.md:51` |
| Low | Stray `criteria.bed` at the end of a comment line | `scripts/docs-partners.js:4` |

Both are cosmetic. Neither affects the checker's behaviour (verified: the checker
still exits 0 with them present, and the typo sits in prose the checker does not
assert on). They are recorded here because this audit read the files, not because
they carry risk. Fixing them is a document edit, which needs the user's instruction
rather than a security remediation.

## 7. Traceability

`traceability.json` contains **no security NFR rows** — the spec's Security Baseline
section declares CON-519 and CON-520 as REQUIRED and the remaining eight controls as
NOT APPLICABLE, but the twelve traceability rows map only FR-001..FR-008 and
NFR-001..NFR-003. There is therefore nothing for this audit to update, and no row was
modified. This is consistent with the review's finding that all twelve rows are
`not_started` with empty `testIds`.

**Governance gap (carried forward, not created here):** CON-519 and CON-520 are
REQUIRED controls with no linked requirement, task, or AC in `traceability.json`.
Per the Security Baseline Traceability Rule this is a traceability gap for
`/sdrive:propose` to close in a future version. It is recorded, not fixed, because
editing traceability outside a security NFR row is out of this command's scope.

## 8. Verdict

**PASS — no security vulnerabilities.** The feature adds no runtime surface, no
dependency, no secret, and no external link. Its documents actively encode the
privacy rules they are subject to (role-not-person, written consent, redaction
checklist), and the checker that enforces those rules was proven to fail when they
are broken.

No code changes were made by this audit. Nothing was committed or pushed.
