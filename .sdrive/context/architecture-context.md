# Architecture Context

> Context file for `.sdrive/constitution.md`.
> Every item below is an observed pattern or an explicit design constraint in the
> repository. Inferred guidance is labelled `assumption`.

## Observed Project Facts

### Workspace shape

```
Packages/                 # repository root; the SpecDrive root
  Cargo.toml              # [workspace] resolver = "2", members = ["crates/*"]
  crates/
    dado/                 # the only implemented crate
  catalogue/              # provider and design Markdown only
    00_provider_index.md  # index of the 26 provider modules
    01_wirekit.md ... 37_signedlink.md
```

- The workspace is a single Cargo workspace rooted at the repository root.
  Source: `Cargo.toml`.
- Workspace-level package settings (`edition`, `rust-version`, `license`) are
  inherited by every crate. Source: `Cargo.toml` `[workspace.package]`.
- The only crate today is `dado`, and it inherits all three settings with
  `edition.workspace = true`, `rust-version.workspace = true`,
  `license.workspace = true`. Source: `crates/dado/Cargo.toml`.

### Package boundaries

The repository states one job per package:

| Package | Single responsibility | Source |
|---|---|---|
| `wirekit` | HTTP clients for external APIs | `catalogue/01_wirekit.md` |
| `syncforge` | Offline replay and conflict handling | `catalogue/02_syncforge.md` |
| `quota-ledger` | Shared quota state across processes | `catalogue/29_quota.md` |
| `hookgate` | Inbound webhook state (delivery, dedupe, replay, outbox) | `catalogue/30_webhook_gateway.md` |
| `platform-core` | Generic provider traits and registry | `catalogue/31_platform_core.md` |
| `ffscene` | Video composition to FFmpeg graph | `catalogue/32_ffscene.md` |
| `mixbus` | Real-time audio graph | `catalogue/33_mixbus.md` |
| `replay-recorder` | Track capture and replay ring | `catalogue/34_replay_recorder.md` |
| `studio-protocol` | Control-plane messages and state machine | `catalogue/35_studio_protocol.md` |
| `rtmp-publish` | Egress destination model and health | `catalogue/36_rtmp_publish.md` |
| `signedlink` | Signed links and attribution | `catalogue/37_signedlink.md` |

`README.md` states: "Do not merge them into a framework."

### Provider clients are modules, not crates

- The 26 provider clients are modules inside `wirekit`, gated by feature flags.
  Source: `catalogue/00_provider_index.md`, `catalogue/01_wirekit.md`.
- Adding a module must not create a duplicate provider client.
  Source: `catalogue/01_wirekit.md` "Proposed modules beyond the 26".
- A module is a candidate, not a contract, until it has a design file following
  the `03`-`28` pattern. Source: `catalogue/01_wirekit.md`.

### Why stateful packages are separate

- `quota-ledger` is separate because `wirekit` must not force a database and a
  tenant model on every consumer. Source: `catalogue/29_quota.md`.
- `hookgate` is separate because a provider client must not force a database, an
  outbox, and a queue, and a webhook edge must not reimplement provider signature
  quirks. Source: `catalogue/30_webhook_gateway.md`.
- `platform-core` is a trait crate that ships no product vocabulary, no OAuth
  implementation, no quota client, and no webhook client.
  Source: `catalogue/31_platform_core.md`.

### Direction of dependencies

```
consuming app / adapter crate
        |
        +--> platform-core   (traits + registry; no product nouns)
        +--> wirekit         (HTTP clients; no app domain types)
        +--> syncforge       (queue + transport traits)
        +--> quota-ledger    (state; optional Postgres/Redis)
        +--> hookgate        (state; optional axum)
        +--> media crates    (ffscene, mixbus, replay-recorder, studio-protocol, rtmp-publish)
        +--> signedlink
```

- `platform-core` never learns a product's nouns; the adapter never re-implements
  the registry. Source: `catalogue/31_platform_core.md`.
- Registration is explicit; there is no global mutable singleton.
  Source: `catalogue/31_platform_core.md`.
- `wirekit` and `platform-core` answer different questions: "how do I call
  Twilio?" versus "which providers does this tenant have and what can each do?".
  Source: `catalogue/31_platform_core.md`.

### Purity and I/O boundaries

- `dado` never opens a socket, reads a URL, spawns a runtime, or writes a file.
  Callers fetch; `dado` parses. Source: `crates/dado/README.md`,
  `crates/dado/src/lib.rs`.
- `ffscene` compiles a scene; it does not encode. A separate process runs FFmpeg.
  Source: `catalogue/32_ffscene.md`.
- `mixbus` processes buffers; it does not open audio devices, decode files, or
  push packets. Source: `catalogue/33_mixbus.md`.
- `replay-recorder` records; it does not encode video for delivery and does not
  stream. Source: `catalogue/34_replay_recorder.md`.
- `studio-protocol` defines messages and a state machine; it does not carry
  media. Source: `catalogue/35_studio_protocol.md`.
- `rtmp-publish` models a destination; it does not ingest, encode, or speak the
  wire protocol. Source: `catalogue/36_rtmp_publish.md`.

### State-machine and verification patterns

- Every `studio-protocol` transition is a pure function of
  `(state, message, actor)`, which makes replay and property testing possible.
  Source: `catalogue/35_studio_protocol.md`.
- `hookgate` preserves the raw body bytes and never re-serializes before
  verification. Source: `catalogue/30_webhook_gateway.md`.
- `rtmp-publish` redacts the stream key in `Debug`/`Display` and never implements
  `Serialize` for the key field. Source: `catalogue/36_rtmp_publish.md`.
- `signedlink` treats the signature, not the short code, as the authorisation.
  Source: `catalogue/37_signedlink.md`.

### Feature-flag pattern in `dado`

| Feature | Module | Optional dependencies | Source |
|---|---|---|---|
| `yaml` | `dado::yaml` | `serde`, `winnow`, `indexmap` | `crates/dado/Cargo.toml` |
| `diag` | `dado::diag` | none | same |
| `graph` | `dado::graph` | none | same |
| `emit` | `dado::emit` | none | same |
| `glob` | `dado::glob` | none | same |

- `default = []`; every module is opt-in.
- Optional dependencies are enabled with `dep:` syntax, so they do not leak as
  implicit features. Source: `crates/dado/Cargo.toml`.

## Standard Best Practices (from MCP/skills)

- Cargo features are additive: enabling a feature must not disable
  functionality, and any combination of features should compile.
  Source: <https://doc.rust-lang.org/cargo/reference/features.html>.
- Feature sets should be documented at the top of `lib.rs`.
  Source: <https://doc.rust-lang.org/cargo/reference/features.html>.
- Rust API Guidelines: types should be `Send`/`Sync` where possible, public
  structs should have private fields, and traits that may be used as trait
  objects should be object-safe.
  Source: <https://rust-lang.github.io/api-guidelines/checklist.html>.
- No Context7 or project skill files were available in this environment.

## Assumptions

- [to be confirmed] Whether `wirekit` will be a single crate with 26+ features or
  split into a companion crate if the feature count becomes a release bottleneck
  (the docs leave this open).
- [to be confirmed] Whether `platform-core` should use `sqlx`/`axum` as optional
  features or stay trait-only in the first release.
- [to be confirmed] The concrete dependency direction for media crates is not
  specified beyond "they compose at the output, not inside one graph type".
