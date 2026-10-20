# Project Constitution

> **Version:** 1.1.0
> **Last Updated:** 2026-10-20
> **Status:** Active

This document is the single source of truth for all project standards, rules, and constraints. All AI agents and developers MUST adhere to these rules.

## 1. Tech Stack & Architecture (CON-1xx)

- **CON-101:** The repository MUST use one Cargo workspace rooted at the repository root with members declared as `crates/*`. <!-- source: .sdrive/context/architecture-context.md §Workspace shape; Cargo.toml -->
- **CON-102:** Every crate MUST inherit `edition = "2021"`, `rust-version = "1.85"`, and `license = "MIT OR Apache-2.0"` from `[workspace.package]`. Overriding them requires a constitution amendment. <!-- source: .sdrive/context/architecture-context.md §Workspace shape; Cargo.toml -->
- **CON-103:** Every package MUST have exactly one job. Packages MUST NOT be merged into a framework. <!-- source: .sdrive/context/project-overview.md §Purpose; README.md -->
- **CON-104:** Provider integrations MUST ship as feature-flagged modules of `wirekit`. A provider MUST NOT become a separate crate without a constitution amendment. <!-- source: .sdrive/context/architecture-context.md §Provider clients are modules, not crates; catalogue/00_provider_index.md -->
- **CON-105:** A new provider module MUST have a design file following the `03`-`28` pattern before implementation begins. <!-- source: .sdrive/context/architecture-context.md §Provider clients are modules, not crates; catalogue/01_wirekit.md -->
- **CON-106:** Public crates MUST NOT contain application domain types, billing rules, product UI, or vendor workflow code. Those belong in the consuming application. <!-- source: .sdrive/context/project-overview.md §Purpose; README.md; catalogue/31_platform_core.md -->
- **CON-107:** Stateful concerns (quota, webhook delivery state, token storage) MUST live in their own crates and MUST NOT be added to `wirekit`. <!-- source: .sdrive/context/architecture-context.md §Why stateful packages are separate; catalogue/29_quota.md; catalogue/30_webhook_gateway.md -->
- **CON-108:** `dado` MUST remain I/O-free: no sockets, no URLs, no runtime, and no file writes. Callers fetch; `dado` parses. <!-- source: .sdrive/context/architecture-context.md §Purity and I/O boundaries; crates/dado/README.md; crates/dado/src/lib.rs -->
- **CON-109:** Public crates MUST NOT expose a global mutable singleton. Registration MUST be explicit. <!-- source: .sdrive/context/architecture-context.md §Direction of dependencies; catalogue/31_platform_core.md -->
- **CON-110:** A public crate MUST NOT depend on a consuming application. Dependencies point from the application to the crate. <!-- source: .sdrive/context/project-overview.md §Purpose; .sdrive/context/architecture-context.md §Direction of dependencies -->
- **CON-111:** A crate name MUST be checked on crates.io immediately before publication. A published name is permanent and MUST NOT be reused. <!-- source: .sdrive/context/project-overview.md §Publishing status; README.md -->

## 2. Coding Standards (CON-2xx)

- **CON-201:** Every crate MUST declare `#![forbid(unsafe_code)]`. Any exception MUST be justified in the crate's design document and approved by a constitution amendment. <!-- source: .sdrive/context/code-standards.md §Crate-level lint attributes; crates/dado/src/lib.rs -->
- **CON-202:** Every crate MUST declare `#![deny(missing_docs)]`, and every public item MUST have a rustdoc comment. <!-- source: .sdrive/context/code-standards.md §Crate-level lint attributes; crates/dado/src/lib.rs -->
- **CON-203:** Every module MUST start with a `//!` doc comment that states its scope and its non-goals. <!-- source: .sdrive/context/code-standards.md §Documentation style; crates/dado/src/*.rs -->
- **CON-204:** Optional functionality MUST be behind additive Cargo features. Enabling a feature MUST NOT disable another, and the default feature set MUST be documented. <!-- source: .sdrive/context/code-standards.md §Feature style; https://doc.rust-lang.org/cargo/reference/features.html -->
- **CON-205:** Error types MUST expose a stable code and implement `Display` and `std::error::Error`. A published code MUST NOT be reused with a different meaning. <!-- source: .sdrive/context/code-standards.md §Error style; crates/dado/src/diag.rs; crates/dado/src/yaml.rs -->
- **CON-206:** Unsupported or unknown input constructs MUST produce a positioned error. They MUST NOT be silently guessed or ignored. <!-- source: .sdrive/context/code-standards.md §YAML contract; crates/dado/src/yaml.rs -->
- **CON-207:** Every provider client MUST map failures into the shared `WirekitError` enum. A provider MUST NOT introduce a second error type. <!-- source: .sdrive/context/code-standards.md §Error, retry, and rate-limit contract; catalogue/01_wirekit.md -->
- **CON-208:** HTTP retries MUST use exponential backoff with jitter, 3 attempts by default, retry on 5xx/429/network errors, honor `Retry-After`, and MUST NOT retry other 4xx. <!-- source: .sdrive/context/code-standards.md §Error, retry, and rate-limit contract; catalogue/01_wirekit.md -->
- **CON-209:** Rate limiting MUST be a per-provider token bucket with per-provider overrides and tracing metrics. <!-- source: .sdrive/context/code-standards.md §Error, retry, and rate-limit contract; catalogue/01_wirekit.md -->
- **CON-210:** HTTP clients MUST share one pooled `reqwest::Client` per process, unless the caller explicitly injects its own client. <!-- source: .sdrive/context/code-standards.md §Error, retry, and rate-limit contract; catalogue/01_wirekit.md -->
- **CON-211:** Every outbound request MUST emit tracing events for request start, request finish (with duration), retry, rate-limit hit, and error. <!-- source: .sdrive/context/code-standards.md §Error, retry, and rate-limit contract; catalogue/01_wirekit.md -->
- **CON-212:** OAuth flows MUST use Authorization Code + PKCE. The implicit flow MUST NOT be used. Integrations MUST request the minimum scopes required. <!-- source: .sdrive/context/code-standards.md §Protocol and payload handling; catalogue/28_google_oauth.md -->
- **CON-213:** HTTP 200 MUST NOT be treated as success when the API reports errors in the body, such as a GraphQL `errors[]` array or Slack `ok: false`. <!-- source: .sdrive/context/code-standards.md §Protocol and payload handling; catalogue/14_jobber.md; catalogue/23_slack.md -->
- **CON-214:** Secrets MUST be redacted in `Debug`, `Display`, logs, traces, and error payloads. A crate MUST NOT implement `Serialize` for a secret field. <!-- source: .sdrive/context/code-standards.md §Secret handling; catalogue/36_rtmp_publish.md; catalogue/15_sage_intacct.md; catalogue/24_deepgram.md -->
- **CON-215:** Webhook signature verification MUST operate on the raw request bytes. The body MUST NOT be re-serialized before verification. <!-- source: .sdrive/context/code-standards.md §Protocol and payload handling; catalogue/30_webhook_gateway.md -->
- **CON-216:** Public APIs MUST follow the Rust API Guidelines: RFC 430 casing, `as_`/`to_`/`into_` conversions, common trait implementations, and a non-empty `Debug` for every public type. <!-- source: .sdrive/context/code-standards.md §Standard Best Practices; https://rust-lang.github.io/api-guidelines/checklist.html -->

## 3. UI/UX Standards (CON-3xx)

- **CON-301:** This repository MUST NOT ship application UI, view code, or framework-coupled presentation components. Crates expose typed data; the consuming application renders it. <!-- source: .sdrive/context/ui-context.md §This repository has no UI layer -->
- **CON-302:** UI obligations in the provider docs (Yelp attribution, unverified-location onboarding, disabled-button UX) are the consuming application's responsibility and MUST NOT be presented as library features. <!-- source: .sdrive/context/ui-context.md §UI obligations that belong to the consuming app; catalogue/19_yelp.md; catalogue/06_google_business.md; catalogue/35_studio_protocol.md -->
- **CON-303:** No Figma design data or design tokens are defined in this repository. Any future design system MUST be recorded in `.sdrive/context/ui-context.md` before UI code is added. <!-- source: .sdrive/context/ui-context.md §Figma and design tokens -->

## 4. Testing & Quality (CON-4xx)

- **CON-401:** Test framework is Rust's built-in test harness (`cargo test`). <!-- source: .sdrive/context/code-standards.md §Tests; Cargo.toml -->
- **CON-402:** Run tests with `cargo test --workspace --all-features` from the repository root. <!-- source: .sdrive/context/code-standards.md §Tests; Cargo.toml -->
- **CON-403:** Unit tests live in inline `#[cfg(test)]` modules. Integration tests live under `crates/<crate>/tests/`. <!-- source: .sdrive/context/code-standards.md §Tests -->
- **CON-404:** Every acceptance criterion MUST have at least one test. <!-- source: SpecDrive -->
- **CON-405:** Test names MUST include the acceptance criterion ID (e.g., `AC-001`). <!-- source: SpecDrive -->
- **CON-406:** Test files MUST be organized by feature, nested under the crate's test folder: `crates/<crate>/tests/<feature>/<subfeature>.rs`. The `<feature>` segment MUST match the SpecDrive feature folder name (kebab-case). If the project adopts a flat test layout, ask the user before restructuring. <!-- source: SpecDrive; .sdrive/context/code-standards.md §Tests -->

## 5. Security & Compliance (CON-5xx)

- **CON-501:** AI agents MUST NOT read, modify, or commit files containing secrets or private information. <!-- source: Universal Security Default -->
- **CON-502:** AI agents MUST treat the following file patterns as OFF-LIMITS: `.env`, `.env.*`, `*.pem`, `*.key`, `credentials.json`, `config/secrets/*`, `.git/config`. <!-- source: Universal Security Default -->
- **CON-503:** AI agents MUST respect `.gitignore` patterns and treat ignored files as private and off-limits. <!-- source: Universal Security Default -->
- **CON-504:** If a task requires environment configuration, AI agents MUST create or update `.env.example` only, NEVER `.env` or `.env.local`. <!-- source: Universal Security Default -->
- **CON-505:** AI agents MUST NEVER commit actual secrets, API keys, passwords, or tokens to the repository, even in test files or comments. <!-- source: Universal Security Default -->

### Security Baseline (CON-51x)

Every feature that touches authentication, user input, or an HTTP surface MUST address each control below. `/sdrive:propose` expands the REQUIRED controls into concrete tasks; `/sdrive:apply` builds them; `/sdrive:secure` verifies them.

| ID | Control | Status | Rationale |
|----|---------|--------|-----------|
| CON-511 | Authentication | REQUIRED | Crates acquire and refresh provider credentials (`wirekit` auth schemes, `platform-core` token store, `hookgate` signature verification). |
| CON-512 | Authorization | REQUIRED | `quota-ledger`, `hookgate`, and `platform-core` must isolate tenants and check capabilities server-side. |
| CON-513 | Security Headers (Helmet) | NOT APPLICABLE | No crate owns an HTTP server or response pipeline; axum integrations document header policy but do not impose it. |
| CON-514 | Rate Limiting | REQUIRED | `wirekit` ships a per-provider token bucket and `quota-ledger` enforces shared provider quota across processes. |
| CON-515 | Input Validation | REQUIRED | Every crate parses untrusted input: provider JSON, webhook bytes, YAML, URLs, and destination shapes. |
| CON-516 | NoSQL / Injection Protection | REQUIRED | Optional Postgres/Redis backends in `quota-ledger` and `hookgate` must use parameterized queries and commands. |
| CON-517 | XSS Protection | NOT APPLICABLE | No crate renders HTML or user-facing markup; `signedlink` returns redirects, not pages. |
| CON-518 | CSRF Protection | NOT APPLICABLE | No crate owns a browser session or cookie-based state; signatures and tokens are the security boundary. |
| CON-519 | Environment Security | REQUIRED | Credentials and signing keys come from caller-supplied config or secret stores; `.env` files are never committed. |
| CON-520 | HTTPS / HSTS | REQUIRED | HTTP clients must use TLS and reject non-HTTPS base URLs; server integrations must document HSTS at the deployment edge. |

<!-- source: SpecDrive Security Baseline; .sdrive/context/project-overview.md; .sdrive/context/architecture-context.md; .sdrive/context/code-standards.md -->

## 6. Git Workflow & Process (CON-6xx)

- **CON-601:** Commits created by SpecDrive commands MUST use `specdrive(<command>): <summary>`. <!-- source: .sdrive/context/ai-workflow-rules.md §Commit and push convention; .sdrive/agents/01-constitution.md -->
- **CON-602:** No feature may be marked complete unless `sdrive verify <feature>` returns `pass` or `warn`. <!-- source: SpecDrive -->
- **CON-603:** Every feature MUST have a version folder (`v1`, `v2`, ...) before implementation begins. <!-- source: SpecDrive -->
- **CON-604:** New versions MUST only be created for structural changes (new workflow, new user story, behavior change). Minor edits stay in the current version. <!-- source: SpecDrive -->
- **CON-605:** All agents MUST operate within the feature's `currentVersion` folder only. Older versions MUST NOT be modified. <!-- source: SpecDrive -->
- **CON-606:** ONE BRANCH PER FEATURE (non-negotiable). Each feature MUST be implemented on its own dedicated branch. Two features MUST NEVER share a branch, and a feature MUST NEVER be implemented on another feature's branch. <!-- source: SpecDrive Enterprise Branching Policy -->
- **CON-607:** The feature branch MUST be named `feature/<feature-name>` using the SpecDrive feature folder name (kebab-case), e.g. `feature/user-login`. If that name is taken, append `-v2`, `-v3`, ... The resolved branch name MUST be reported to the user. <!-- source: SpecDrive Enterprise Branching Policy -->
- **CON-608:** Agents MUST NEVER implement directly on the repository's default or protected branch. The default branch MUST be resolved dynamically and NEVER assumed to be `main`. Implementing on a protected branch is a blocking violation. <!-- source: SpecDrive Enterprise Branching Policy -->
- **CON-609:** A feature branch MUST be created from an approved base branch, and the working tree MUST be clean (`git status --porcelain` empty) before any branch switch or creation. A dirty tree MUST block the operation. Creation happens in `/sdrive:propose`, before any spec file is written; `/sdrive:apply` only resumes the recorded branch and MUST NOT create one. <!-- source: SpecDrive Enterprise Branching Policy -->
- **CON-610:** The feature branch MUST be deleted after its merge is confirmed - locally (`git branch -d`) and on the remote (`git push origin --delete`) - unless the user explicitly declines. The merge target branch MUST NEVER be deleted. <!-- source: SpecDrive Enterprise Branching Policy -->
- **CON-611:** A branch MUST be merged as a single review unit. `/sdrive:archive` MUST refuse to merge a branch that contains commits belonging to other in-flight features. <!-- source: SpecDrive Enterprise Branching Policy -->
- **CON-612:** A feature branch MUST be based on the branch it will eventually merge into, so that `git log <target-branch>..<base-branch>` is empty at creation. The base is chosen from the list of available branches and recorded in `/sdrive:propose`, before the spec exists, so the choice is never made blind. Basing a feature branch on another in-flight feature's branch stacks the two and makes this feature unarchivable until the other one is merged; that must be reported with its consequences and explicitly confirmed, never done silently. Dependent work MUST become a new version of the same feature (`sdrive version:new`), not a stacked feature. <!-- source: SpecDrive Enterprise Branching Policy -->
- **CON-613:** Agents MUST stage only the files the command created or modified. They MUST NOT use `git add -A` when unrelated work is present. <!-- source: .sdrive/context/ai-workflow-rules.md §Commit and push convention; .sdrive/agents/01-constitution.md -->
- **CON-614:** Agents MUST NOT force-push, and MUST NOT amend a commit they did not create in the current run. <!-- source: .sdrive/context/ai-workflow-rules.md §Commit and push convention; .sdrive/agents/01-constitution.md -->

## 7. AI Agent Rules (CON-7xx)

- **CON-701:** AI agents MUST read `.sdrive/commands/manifest.json` to resolve commands and MUST NOT hardcode the command list. <!-- source: .sdrive/context/ai-workflow-rules.md §Command entry point; AGENTS.md -->
- **CON-702:** AI agents MUST stop at every human approval gate and MUST NOT write context or constitution files before approval. <!-- source: .sdrive/context/ai-workflow-rules.md §Approval and roles; .sdrive/agents/01-constitution.md -->
- **CON-703:** AI agents MUST run `node .sdrive/scripts/validate-governance.js` after writing governance artifacts. <!-- source: .sdrive/context/ai-workflow-rules.md §Validators and evidence -->
- **CON-704:** AI agents MUST NOT invent project facts. Unknown facts MUST be marked `[to be confirmed]` or raised to the user. <!-- source: .sdrive/context/ai-workflow-rules.md §Standard Best Practices; .sdrive/agents/01-constitution.md -->
- **CON-705:** Every rule and context claim MUST carry a source reference. <!-- source: .sdrive/context/ai-workflow-rules.md §Standard Best Practices; .sdrive/agents/01-constitution.md -->
- **CON-706:** AI agents MUST NOT generate, suggest, or commit secrets. Use placeholders such as `<your-api-key>` or `.env.example`. <!-- source: .sdrive/context/ai-workflow-rules.md §Standard Best Practices; .sdrive/agents/01-constitution.md -->
- **CON-707:** AI agents MUST read the relevant files under `.sdrive/skills/` before generating governance artifacts. <!-- source: .sdrive/context/ai-workflow-rules.md §Standard Best Practices; .sdrive/agents/01-constitution.md -->

---

## Changelog

| Version | Date | Change Summary | Source |
|---------|------|----------------|--------|
| 1.0.0   | 2026-10-20 | Initial merge of repository discovery, architecture, code standards, UI scope, testing, security baseline, git workflow, and AI agent rules | context files |
