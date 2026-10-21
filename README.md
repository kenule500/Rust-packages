# GIA Packages

The public Rust packages that GIA and other projects build and consume. This is
their standalone project; GIA is one consumer, not the owner of the ecosystem.
The packages are designed to be open source (MIT OR Apache-2.0) and published
to crates.io.

## Where this lives

```
C:\Users\KENULE-NWINEE\Documents\vscode-projs\Projects\Packages
```

Package docs, implementation, releases, and crates.io publishing happen here.
Other projects — including GIA and OmniStream — consume the crates; they do not
build them in their own repositories. The Cargo workspace and crates live at the
repository root; `catalogue/` holds only the provider and design Markdown docs.
The provider docs in `catalogue/` are the current integration contract; the GIA
catalog copy is pending.

| # | Package | What it is | Doc |
|---|---|---|---|
| 00 | Provider index | All 26 provider docs, status, and “already online?” answers | [00_provider_index.md](catalogue/00_provider_index.md) |
| 01 | `wirekit` | Typed HTTP clients for the external APIs a Rust app needs | [01_wirekit.md](catalogue/01_wirekit.md) |
| 02 | `syncforge` | Offline-first sync engine for client and server | [02_syncforge.md](catalogue/02_syncforge.md) |
| 03–28 | Provider docs | One design file per wirekit module, Twilio through Google OAuth | [00_provider_index.md](catalogue/00_provider_index.md) |
| 29 | `quota-ledger` | Per-tenant, per-provider quota reservation and ledger | [29_quota.md](catalogue/29_quota.md) |
| 30 | `hookgate` | Inbound webhook edge: verify, dedupe, replay-protect, forward | [30_webhook_gateway.md](catalogue/30_webhook_gateway.md) |
| 31 | `platform-core` | Generic provider traits: IDs, capability type, registry, token-store trait | [31_platform_core.md](catalogue/31_platform_core.md) |
| 32 | `ffscene` | Scene graph → FFmpeg filter graph, encoder ladder, GPU probe | [32_ffscene.md](catalogue/32_ffscene.md) |
| 33 | `mixbus` | Real-time audio mixing graph: gain, ducking, gate, loudness, plugins | [33_mixbus.md](catalogue/33_mixbus.md) |
| 34 | `replay-recorder` | Multi-track capture, replay ring buffer, segment/tee muxing | [34_replay_recorder.md](catalogue/34_replay_recorder.md) |
| 35 | `studio-protocol` | Preview/program control protocol, roles, whisper, teleprompter | [35_studio_protocol.md](catalogue/35_studio_protocol.md) |
| 36 | `rtmp-publish` | RTMP/SRT/WHIP destination model, key redaction, health, failover | [36_rtmp_publish.md](catalogue/36_rtmp_publish.md) |
| 37 | `signedlink` | Short signed links, UTM builder, expiry, attribution window | [37_signedlink.md](catalogue/37_signedlink.md) |

## Crate names that are already taken

Crates.io names are permanent. Two concepts had to be renamed before publish;
the catalogue file names stay as numbers, not crate names.

| Working name | Published crate | Reason |
|---|---|---|
| `quota` | **`quota-ledger`** | `quota` is a lane-parallel rate limiter (MIT, v0.3.3) |
| `webhook-gateway` | **`hookgate`** | `webhook-gateway` is a dormant GitHub-event ingress (MIT OR Apache-2.0, v2.17.2) |
| `media-engine` | **`ffscene`** | `media_engine` exists (MIT, v0.1.4) |
| `audio-engine` | **`mixbus`** | `audio-engine` exists (MIT OR Apache-2.0, v0.4.5) |
| `recorder` | **`replay-recorder`** | `recorder` exists (MIT, v0.2.1) |

Always check crates.io immediately before reserving a name; availability checked
October 2026.

## What about the 26 provider clients?

They are **wirekit modules**, not separate crates. Twilio, SendGrid, Meta,
NOAA, EagleView, QuickBooks, and the rest ship behind wirekit feature flags:

```toml
wirekit = { version = "0.1", features = ["twilio", "sendgrid", "quickbooks"] }
```

Their technical contracts — structs, methods, auth, base URLs, webhooks — are
the provider docs in `catalogue/`. Their packaging, feature flags, and semver
live with the `wirekit` crate. The status of each one (third-party API online,
existing Rust client, whether it is a separate package) is in
[00_provider_index.md](catalogue/00_provider_index.md), with a dedicated design file per
provider from [03_twilio.md](catalogue/03_twilio.md) to
[28_google_oauth.md](catalogue/28_google_oauth.md).

## Why these are separate from the app

- **Reuse.** Every GIA service that talks to Twilio or Stripe uses the same
  client, the same error type, and the same retry policy.
- **Independent lifecycle.** A provider fix ships as a crate release, not an
  app release.
- **Ecosystem value.** These packages are useful to any Rust developer, not
  just one product. That is deliberate: it creates contributors, reputation,
  and an asset that survives a pivot.
- **One job per package.** `wirekit` is HTTP clients; `syncforge` is offline
  replay; `quota-ledger` is quota state; `hookgate` is inbound webhook state;
  `ffscene` is composition; `mixbus` is audio. Do not merge them into a
  framework.

## Public crates.io, not app-only

| Tier | Where it lives | What it means |
|---|---|---|
| **Public** | crates.io | The 11 packages in the table above — generic, documented, semver, MIT/Apache-2.0 |
| **Public later** | crates.io after API freeze | A package that starts app-local can move here once a second project uses it, its API is stable, and it exposes no app domain types |
| **App-local** | The consuming app's repo | Domain types, billing rules, product UI, proprietary workflows, service composition |

A public crate can still be the primary implementation used inside OmniStream.
“Public” does **not** mean “only for other people”; it means the package is
generic enough to be maintained as a standalone library. App-specific code does
not belong here, because publishing it would create a permanent API and support
burden for a package nobody else can use.

## Reading order

1. [01_wirekit.md](catalogue/01_wirekit.md) — why one HTTP client layer beats twenty.
2. [02_syncforge.md](catalogue/02_syncforge.md) — why offline sync is a distributed
   systems problem, and what the library does about it.
3. [00_provider_index.md](catalogue/00_provider_index.md) — the 26 modules, what is
   online, and what is actually “to build.”
4. [03_twilio.md](catalogue/03_twilio.md) … [28_google_oauth.md](catalogue/28_google_oauth.md) —
   one design file per provider.
5. [29_quota.md](catalogue/29_quota.md) — why provider quota is state, not a local
   token bucket.
6. [30_webhook_gateway.md](catalogue/30_webhook_gateway.md) — why webhook dedupe and
   replay protection belong in front of every provider.
7. [31_platform_core.md](catalogue/31_platform_core.md) — the tenant-scoped substrate
   every platform family shares.
8. [32_ffscene.md](catalogue/32_ffscene.md) — composition as a compiled graph, not a
   black box.
9. [33_mixbus.md](catalogue/33_mixbus.md) — a real-time audio graph with real-time rules.
10. [34_replay_recorder.md](catalogue/34_replay_recorder.md) — isolated tracks and a
    replay ring that survives a crash.
11. [35_studio_protocol.md](catalogue/35_studio_protocol.md) — who may change the
    picture, and when.
12. [36_rtmp_publish.md](catalogue/36_rtmp_publish.md) — destinations, secrets, and
    honest health.
13. [37_signedlink.md](catalogue/37_signedlink.md) — signed links and attribution that
    fail closed.

## Status

Blueprint. None of the packages is published yet. `wirekit` and `syncforge` are
specified here; implementation starts after the first design partners validate
the field workflow. `quota-ledger` and `hookgate` are standalone because they
own state that `wirekit` must not own. The media and link packages are public
candidates whose APIs must freeze before publish. `dado` 0.1.0 is implemented
and tested; the remaining packages are still blueprints.

The M3 substrate crates are in development on `feature/m3-substrate`:

| Crate | Status | Responsibility |
|---|---|---|
| `platform-core` | in development, 0.1.0 | Tenant/provider identity, capabilities, explicit registry, token-store contract |
| `quota-ledger` | in development, 0.1.0 | Reserve/commit/release quota state with an optional Postgres backend |
| `hookgate` | in development, 0.1.0 | Raw-byte webhook verification, dedupe, replay window, outbox, dead-letter |
| `syncforge` | in development, 0.1.0 | Durable action queue, UUID v7 idempotency, conflict strategies |

None of the four is published. Publication is a manual post-merge step.

