# AI Workflow Rules

> Context file for `.sdrive/constitution.md`.
> These are the operating rules for AI agents and humans working in this
> SpecDrive project. Sources are the installed SpecDrive framework and the root
> `AGENTS.md`, not inference.

## Observed Project Facts

### Command entry point

- The root `AGENTS.md` requires every `/sdrive:*` command to:
  1. read `.sdrive/commands/manifest.json`;
  2. find the matching command;
  3. read the `agentFile` listed there;
  4. follow that agent's instructions exactly;
  5. run validators after writing files;
  6. stop at human approval gates.
- Commands and agent files must never be hardcoded.
  Source: `AGENTS.md`.

### Installed commands

Source: `.sdrive/commands/manifest.json`.

| Command | Agent | Validators | Approval gates |
|---|---|---|---|
| `/sdrive:constitution` | `01-constitution.md` | `governance` | Gate 0 (context), Gate 1 (rules) |
| `/sdrive:product-plan` | `12-product-plan.md` | none | none |
| `/sdrive:propose` | `02-specification.md` | `governance` | none |
| `/sdrive:update-propose` | `13-update-propose.md` | `governance` | none |
| `/sdrive:design` | `03-uiux.md` | `governance` | none |
| `/sdrive:sync` | `04-cascade.md` | `governance` | none |
| `/sdrive:skills` | `05-discover-skills.md` | none | none |
| `/sdrive:docs` | `06-documentation.md` | `governance` | none |
| `/sdrive:apply` | `07-implementation.md` | `governance` | `specification`, `planning` |
| `/sdrive:performance` | `08-performance.md` | `governance` | none |
| `/sdrive:debug` | `15-debug.md` | `governance` | none |
| `/sdrive:review` | `09-review-complete.md` | `governance` | `implementation` |
| `/sdrive:archive` | `14-archive.md` | `governance` | `review` |
| `/sdrive:secure` | `10-security.md` | `governance` | none |
| `/sdrive:test` | `11-test.md` | `governance` | none |
| `/sdrive:onboard` | `00-onboarding.md` | `governance` | none |

### Approval and roles

- `.sdrive/team.json` lists `kenule500` as `admin`.
- Approval roles: `admin`, `reviewer`.
- Implementation roles: `developer`, `admin`.
- `.sdrive/config.json` requires test evidence, forbids stale evidence, and
  requires structured review.
- No feature exists yet: `.sdrive/workflow-state.json` is `{ "features": [] }`.

### Branching and versioning (from the installed constitution template)

The `/sdrive:constitution` agent ships a mandatory enterprise branching policy:

- One branch per feature; never share or reuse a feature branch.
- Branch name `feature/<feature-name>` (kebab-case), with `-v2`/`-v3` on
  collision.
- Never implement on the default or protected branch; resolve the default branch
  dynamically.
- Create the branch in `/sdrive:propose` from an approved base branch with a
  clean working tree.
- Delete the feature branch after a confirmed merge.
- Merge one feature per review unit; refuse to merge a branch containing another
  in-flight feature's commits.
- Base the feature branch on the branch it will merge into; stacking must be
  reported and explicitly confirmed. Dependent work becomes a new version of the
  same feature, not a stacked feature.

Source: `.sdrive/agents/01-constitution.md`, `<constitution-template>`
CON-606 through CON-612.

### Version folder policy (from the installed constitution template)

- Every feature has a version folder (`v1`, `v2`, ...).
- New versions only for structural changes (new workflow, new user story,
  behavior change). Minor edits stay in the current version.
- All agents operate only in the feature's `currentVersion` folder.

Source: `.sdrive/agents/01-constitution.md`, `CON-603` to `CON-605`.

### Validators and evidence

- `.sdrive/scripts/validate-governance.js` validates `config.json`,
  `workflow-state.json`, and each feature's `spec.json`, `plan.json`,
  `tasks.json`, `traceability.json`, `test-evidence.json`, and `review.json`
  against JSON Schema.
- The CLI is run as `node .sdrive/scripts/validate-governance.js`.
- `sdrive verify <feature>` is the per-feature verification command. No feature
  exists yet.
- The project's `.sdrive/config.json` requires test evidence and structured
  review, and disallows stale evidence.

### Commit and push convention (from the installed command footer)

- Stage only the files the command created or modified. Never `git add -A` when
  unrelated work is present.
- Commit format: `specdrive(<command>): <summary>`, for example
  `specdrive(propose): add user-login specification`.
- Push the current branch to its upstream; set upstream once if missing.
- Never force-push and never amend a commit created by another run.
- If Git or a remote is unavailable, report it and leave the tree intact.

Source: `.sdrive/agents/01-constitution.md`, `<git-commit-and-push>`.

### Repository-specific workflow facts

- The repository's default branch is `main`; the only remote is `origin`.
- The working tree is centered on the Cargo workspace at the repository root;
  provider and design Markdown lives under `catalogue/`.
- CI is configured in `.github/workflows/ci.yml`; there is no pre-commit hook
  installed and no release automation.
- `.gitignore` ignores `Cargo.lock`, `target/`, and `*.rs.bk`.

## Standard Best Practices (from MCP/skills)

- Spec-driven development: specify, plan, task, implement, verify, review, and
  archive as separate gates. This is the model the installed framework encodes.
- Human approval must precede constitution changes, context-file writes, and
  rule changes. Source: `.sdrive/agents/01-constitution.md`.
- Every rule needs a source reference; no invented project facts.
  Source: `.sdrive/agents/01-constitution.md`.
- No Context7 or project skill files were available in this environment.

## Assumptions

- [to be confirmed] Whether protected-branch rules are enforced by GitHub branch
  protection, since no workflow or branch-protection export exists in the repo.
- [to be confirmed] Whether a CI pipeline will be added; none exists today.
- [to be confirmed] Whether the `developer` and `reviewer` roles will be filled
  beyond the single `admin` member.
- [to be confirmed] Whether `sdrive` is expected to be run from the repository
  root (`Packages/`). The commands in this session were run from `Packages/`.
