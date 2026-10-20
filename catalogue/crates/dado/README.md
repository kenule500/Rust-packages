# dado

**The base layer for config-driven tools.** A strict YAML parser, a diagnostics
renderer, a dependency graph, a code emitter, and a glob matcher — one crate,
five feature flags, zero dependencies by default.

```toml
[dependencies]
dado = { version = "0.1.0", features = ["yaml"] }
```

Enable every module when you need the whole toolkit:

```toml
[dependencies]
dado = { version = "0.1.0", features = ["yaml", "diag", "graph", "emit", "glob"] }
```

## Example

```rust
use std::collections::BTreeMap;

fn main() -> Result<(), dado::yaml::Error> {
    let input = "\
name: gateway
region: us-east-1
";

    let config: BTreeMap<String, String> = dado::yaml::from_str(input)?;
    assert_eq!(config["name"], "gateway");

    Ok(())
}
```

Typed configuration uses `serde` derive, as with any serde format. Add
`serde = { version = "1", features = ["derive"] }` to your dependencies:

```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    name: String,
    retries: u32,
}

let config: Config = dado::yaml::from_str(input)?;
```

Errors carry a stable code, a 1-based line and column, and a 0-based byte
offset, so diagnostics can point at the exact input byte:

```rust
match dado::yaml::from_str::<Config>(input) {
    Ok(config) => { /* ... */ }
    Err(error) => {
        eprintln!("{} ({}:{})", error, error.line, error.column);
        // With the `diag` feature: Span::new(error.offset, error.offset + 1)
        let _ = error.offset;
    }
}
```

The error type is independent of the `diag` feature. `diag::Span::new` can use
`error.offset` directly when that feature is enabled.

## Modules

| Feature | Module | What it does |
|---|---|---|
| `yaml` | `dado::yaml` | Strict YAML-1.2-core parsing into `serde` types |
| `diag` | `dado::diag` | Stable codes, source spans, caret rendering |
| `graph` | `dado::graph` | Topological order, cycle detection |
| `emit` | `dado::emit` | Indentation-aware code writer |
| `glob` | `dado::glob` | `*`, `**`, `?` matching |

## Design rules

- **No I/O, ever.** Bytes in, values out. No sockets, no URLs, no runtime, no
  file writes. Callers fetch; `dado` parses.
- **Zero dependencies by default.** Only `yaml` adds `serde`, `winnow`, and
  `indexmap`.
- **Strict over permissive.** Unknown constructs are faults with a positioned
  error, never a silent guess.
- **No framework coupling.** Nothing here knows about any application's
  schema.

## YAML scope

Supported: block mappings and sequences, single-line flow collections, plain
and quoted scalars, literal and folded block scalars with `-`/`+` chomping,
comments, and multi-line plain scalars.

Rejected with an error: tags, directives, anchors, aliases, merge keys,
multi-document streams, complex keys, duplicate keys, tabs used for
indentation, inconsistent indentation, unterminated quotes or flow collections,
and unsupported escape sequences.

Booleans follow YAML 1.2 core — `true`/`false` only. `yes`, `no`, `on`, and
`off` are strings. Integers may be decimal, hexadecimal, octal, or binary.
Floats must be finite; `.inf` and `.nan` are rejected. Nesting is limited to 128
levels, and error messages never echo scalar values.

## Status

`0.1.0` is the baseline release. All five feature-gated modules are implemented;
`yaml` is a strict subset parser rather than a complete YAML 1.2 implementation.
The crate is pre-1.0 and may make breaking changes in minor releases.

## Licence

MIT OR Apache-2.0. See
[LICENSE-MIT](https://github.com/kenule500/Rust-packages/blob/main/LICENSE-MIT)
and
[LICENSE-APACHE](https://github.com/kenule500/Rust-packages/blob/main/LICENSE-APACHE).
