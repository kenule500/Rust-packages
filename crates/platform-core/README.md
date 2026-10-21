# platform-core

Tenant and provider identity, capabilities, an explicit provider registry, and
a token-store contract for multi-tenant Rust services.

`platform-core` is a **trait crate**. It ships no product vocabulary, no OAuth
implementation, no quota client, and no webhook client. Those belong to the
consuming product's adapter crate.

## Features

| Feature | Default | Adds |
|---|---|---|
| `registry` | yes | `Provider` trait, `Registry`, and capability types |
| `memory` | yes | `MemoryTokenStore` for tests and single-binary tools |

Identity types, `Secret`, `TokenStore`, and `IntegrationError` are always
available. `--no-default-features` builds the minimal core.

## Example

```rust
use std::sync::Arc;

use platform_core::{Capability, Provider, ProviderId, ProvisioningMode, Registry};

struct ExampleProvider;

impl Provider for ExampleProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new("example")
    }

    fn capabilities(&self) -> &'static [Capability] {
        const CAPS: &[Capability] = &[Capability {
            key: "messages.send",
            mode: ProvisioningMode::Api,
        }];
        CAPS
    }
}

let mut registry = Registry::new();
registry.register(Arc::new(ExampleProvider)).expect("unique provider");
assert!(registry.get(&ProviderId::new("example")).is_some());
```

## Non-goals

- No global mutable provider registry.
- No token encryption; `TokenStore` leaves storage and KMS integration to the
  adapter.
- No `sqlx` or `axum` features in 0.1.
- No dependency on `wirekit` in this version.

## Related crates

- [`quota-ledger`](../quota-ledger/README.md) - reserves quota using the
  identity types from this crate.
- [`hookgate`](../hookgate/README.md) - inbound webhook verification and
  dedupe using the same identity types.
- [`syncforge`](../syncforge/README.md) - durable action replay with its own
  identity and conflict model.
- [Workspace README](../../README.md) - package table, status, and reading
  order.

## License

MIT OR Apache-2.0.
