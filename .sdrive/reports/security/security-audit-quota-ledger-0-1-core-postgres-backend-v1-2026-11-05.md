# Security Audit Report: quota-ledger 0.1 core reservations and Postgres backend (v1)

## 1. Executive Summary
- **Date:** 2026-11-05
- **Auditor:** AI Security Agent
- **Feature Version:** v1
- **Overall Risk Level:** Medium → **Low** after remediation
- **Total Vulnerabilities Found:** 1 (Critical: 0, High: 0, Med: 1, Low: 0)

This audit was run as the remediation half of review round 1: `/sdrive:review`
already recorded the security finding as `fail`, so it was not re-audited, only
fixed and re-verified.

One dependency finding was raised and **fixed before this audit concluded**:
RUSTSEC-2023-0071 (`rsa` 0.9.10) reached `Cargo.lock` through the `sqlx` facade.
The advisory scores 5.9 (Medium) and the affected crate was **never compiled**
(`cargo tree -i rsa` is empty); review recorded the finding as High because it
made the required CI **Audit** job fail. `cargo audit` now exits 0 and no
advisory remains. No open critical or high finding is left by this command.

## 2. Threat Model Summary
- **Key Assets:** the per-tenant/per-provider quota state (limit, used, held,
  overage) that one tenant MUST NOT observe or spend for another; the Postgres
  connection string, which carries credentials the caller supplies; the
  reservation/idempotency records that keep a retry from holding quota twice;
  and the supply chain of the crate's own build.
- **Trust Boundaries:**
  1. caller application ↔ `quota_ledger::postgres` — the caller supplies an
     already-configured connection; the crate reads no environment variable, no
     `.env`, and no file (CON-519), and `Debug` for the store redacts the pool.
  2. `quota_ledger::postgres` ↔ Postgres — TLS is required on the production
     `connect` path (CON-520); statements are parameterized (CON-516).
  3. tenant ↔ tenant — every store operation is scoped by the tenant in the
     `QuotaKey`, enforced server-side by the SQL predicates (CON-512).
  4. build ↔ crates.io — `Cargo.lock` and any advisory scanner operate across
     this boundary; **this is the boundary the finding sat on.**
- **Attacker capabilities considered:** a hostile or curious co-tenant probing
  another tenant's key; a caller-controlled connection string; a caller-supplied
  metric/window used as a parameter; and a compromised or vulnerable transitive
  dependency entering the build graph.

## 3. Vulnerability Details

### VULN-001: RUSTSEC-2023-0071 (`rsa` 0.9.10) reaches `Cargo.lock` through the `sqlx` facade
- **CVSS Score:** 5.9 (Medium), per the RustSec advisory (no fixed upgrade is
  available). Feature-level severity **High**: it made a required CI job fail.
- **Location:** `crates/quota-ledger/Cargo.toml` (the optional `sqlx` facade
  dependency) → `Cargo.lock` (`rsa 0.9.10`, via `sqlx → sqlx-mysql → rsa`).
- **Description:** `cargo generate-lockfile && cargo audit` exited 1 on
  RUSTSEC-2023-0071 ("Marvin Attack: potential key recovery through timing
  sidechannels"). The advisory is **unreachable from this crate at build time**:
  `cargo tree -p quota-ledger --all-features -i rsa` prints nothing, because only
  the `postgres` feature is enabled. The crate still reached the lockfile because
  the `sqlx` facade declares `sqlx-mysql` and `sqlx-sqlite` as optional
  dependencies **on itself**, and Cargo resolves optional dependencies into
  `Cargo.lock` regardless of the features in play. `.cargo/audit.toml` carries an
  empty `ignore` list, so the Audit job went red at the reviewed commit.
- **Impact:** no runtime impact at the reviewed commit — `rsa` was never
  compiled, so the timing side-channel was not exploitable through this crate.
  The real damage was process and supply chain: a required CI gate failed, and
  enabling any other `sqlx` backend later would have shipped a crate with no
  fixed upgrade into an authentication path.
- **Remediation:** `quota-ledger` now depends on `sqlx-postgres` and `sqlx-core`
  directly, forwarding the runtime/TLS/data-type features the facade used to
  (`_rt-tokio`, `_tls-rustls-ring-webpki`, `chrono`, `uuid`), and restores the
  facade's flat surface with a private `mod sqlx` re-export so no query call
  site or SQL string changed. Applied in
  `specdrive(secure): depend on the sqlx driver crates so rsa leaves the
  lockfile` (`2269ec0`). `Cargo.lock` drops from 314 to 292 resolved crates;
  `sqlx`, `sqlx-mysql`, `sqlx-sqlite` and `rsa` are gone.
- **Status:** Fixed, verified by re-running `cargo audit` (exit 0) and the
  workspace suite (234 passed / 0 failed / 2 ignored).

## 4. Security Baseline

| Control | Status | Evidence |
|---|---|---|
| CON-511 Authentication | NOT APPLICABLE | The crate acquires no credentials; the caller passes an already-configured connection. |
| CON-512 Authorization | PASS | `QuotaKey` carries the tenant and every statement is scoped by it; AC-014 proves a foreign tenant's key cannot read the ledger. |
| CON-513 Security Headers | NOT APPLICABLE | No HTTP server or response pipeline. |
| CON-514 Rate Limiting | PASS | This crate is the shared quota-enforcement control; per-process limiting stays in `wirekit`. |
| CON-515 Input Validation | PASS | `reserve` rejects zero units and non-positive TTLs; `u64`→`i64` conversions and connection-string parsing return typed errors (AC-015, EC-003). |
| CON-516 Injection | PASS | Every statement binds `$n` parameters; AC-015 asserts no `SELECT/INSERT/UPDATE/DELETE` is built with `format!`. The atomic claim is a single conditional `UPDATE` (FR-010). |
| CON-517 XSS | NOT APPLICABLE | No markup is produced. |
| CON-518 CSRF | NOT APPLICABLE | No browser session or cookie-based state. |
| CON-519 Environment Security | PASS | No `std::env`/`.env`/file read in the crate; `Debug for PostgresQuotaStore` redacts the pool. |
| CON-520 HTTPS | PASS | `connect()` forces `PgSslMode::Require`; `connect_insecure_for_tests` is a documented test-only escape hatch — raised in review as a medium, non-blocking observation. |

Traceability cross-check: the CON-51x controls map onto NFR-001 to NFR-004 and
NFR-006, all of which are `complete` in `traceability.json` with linked ACs and
tests. **No traceability row changed status as a result of this fix** — every
security NFR row was already `complete`, and none is regressed. All pre-existing
rows and statuses were preserved.

## 5. Security Regression Verification
- [x] **Dependency audit passed** — `cargo generate-lockfile && cargo audit`
  exits 0; 292 resolved crates, no advisory. The specific regression is proven:
  `findstr rsa Cargo.lock` finds nothing.
- [x] **Functional tests passed** — `cargo test --workspace --all-features`
  exits 0 with 234 passed / 0 failed / 2 ignored, unchanged from the reviewed
  baseline. `cargo fmt --all --check`, `cargo clippy --workspace --all-features
  -- -D warnings`, and `cargo doc --workspace --all-features --no-deps`
  (`RUSTDOCFLAGS=-D warnings`) are all clean, matching CI.
- [x] **Secret scan performed** — see limitations; a manual regex scan
  (`AKIA`, `BEGIN … PRIVATE KEY`, `password =`, `api_key =`, `token =`,
  `postgres://user:pass@`) over every changed file and the feature's governance
  files found no secret. The only connection-string hit was the documentation
  placeholder in `crates/quota-ledger/README.md` (a `user:pass` example, not a
  credential).
- **Limitations:**
  - **`gitleaks` and `trufflehog` are not installed** in this environment, so
    the secret scan is a manual pattern scan, not a tool-verified one. This check
    is reported as *performed with a limitation*, not as tool-verified.
  - **CI's MSRV (1.87) job is still red** — `uuid = ^1.28.0` forces a version
    that requires Rust 1.89. This is a build/MSRV defect, **not a security
    control**, and is out of this command's scope; review routed it to
    `/sdrive:apply` as task **T5.2** (finding **F2**). It is **not** fixed here.
  - `sqlx-core`'s API is documented as semver-exempt, so the driver crates are
    pinned and must be bumped in lockstep (see Recommendations).
  - No dynamic analysis, and no live Postgres: AC-013 proves the concurrency
    invariant on `MemoryQuotaStore` only (RISK-001, unchanged).

## 6. Recommendations
1. **Pin and bump the SQLx driver crates in lockstep.** `sqlx-core` is explicitly
   not semver-stable; treat `sqlx-core` + `sqlx-postgres` as one versioned unit
   and take advisory-driven upgrades deliberately.
2. **Keep `cargo audit` on every push** (CI already runs it after a fresh
   `cargo generate-lockfile`) so a reintroduced transitive advisory fails fast.
3. **Fix F2 before the next review** — lower the `uuid` floor in
   `crates/quota-ledger/Cargo.toml` to a 1.87-buildable version
   (`platform-core` already declares `uuid = "1"`) via `/sdrive:apply` / T5.2.
4. **Harden `connect_insecure_for_tests`** (medium, non-blocking): consider
   gating TLS-disabling behind `#[cfg(test)]` or a private feature so a
   production caller cannot turn TLS off through public API.
5. **Document the reverse migration** for `src/postgres/schema.sql` (low,
   non-blocking) so an adopter has a rollback path.
