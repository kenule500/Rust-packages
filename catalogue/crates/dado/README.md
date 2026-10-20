# dado

**The base layer for config-driven tools.** A strict YAML parser, a diagnostics
renderer, a dependency graph, a code emitter, and a glob matcher — one crate,
five feature flags, zero dependencies by default.

```toml
dado = { version = "0.1", features = ["yaml", "diag", "graph", "emit", "glob"] }
```

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
and quoted scalars, block scalars (`|`, `>` with chomping), comments, and
multi-line plain scalars.

Rejected with an error: tags, directives, anchors, aliases, merge keys,
multi-document streams, tabs used for indentation, duplicate keys.

Booleans follow YAML 1.2 core — `true`/`false` only. `yes`, `no`, `on`, `off`
are strings.

## Status

Scaffold. `diag`, `graph`, `emit`, and `glob` are implemented; `yaml` is the
next piece of work and currently returns `D0001`.

## Licence

MIT OR Apache-2.0.
