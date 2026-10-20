# Security Policy

## Supported versions

Security fixes are provided for the current `0.1.x` release line. The crate is
pre-1.0, so the API may change between minor versions; security fixes will still
be released for the latest published version.

| Version | Supported |
|---|---|
| 0.1.x | Yes |
| older | No |

## Reporting a vulnerability

Please use GitHub private vulnerability reporting:

<https://github.com/kenule500/Rust-packages/security/advisories/new>

Do **not** open a public issue, discussion, or pull request for a suspected
vulnerability. Public disclosure before a fix is available puts users at risk.

Include as much of the following as you can:

- The affected `dado` version and enabled features.
- A minimal reproducer: input YAML, the Rust type being deserialized, and the
  observed result.
- The expected behavior and why the result is a security problem.
- Impact: panic, stack exhaustion, denial of service, data exposure, or another
  concrete effect.
- Your environment: OS, Rust version, and whether the input is untrusted.
- Sanitized logs or a backtrace. Never include real credentials, tokens, or
  personal data.

If you need to share secrets to demonstrate a problem, redact them first. The
maintainers will arrange a private channel if a secret is essential.

## Response targets

These are targets, not guarantees:

- **Acknowledgement:** within 3 business days.
- **Initial triage and severity assessment:** within 10 business days.
- **Fix or mitigation plan:** within 90 days for confirmed issues, with a
  shorter target for critical or actively exploited issues.

You will receive updates when the status changes. If you do not receive an
acknowledgement within the target window, please send a follow-up through the
same private advisory.

## Coordinated disclosure

We ask reporters to keep the report private until a fix or mitigation is
released. We will coordinate a disclosure date with you and credit you in the
advisory unless you prefer to remain anonymous.

## Scope

This policy covers the `dado` crate and the code in this repository, including:

- The strict YAML parser and serde deserializer.
- Panics, unbounded recursion, or excessive resource use caused by untrusted
  input.
- Error messages or diagnostics that reveal scalar values or credentials.
- The CI and release configuration, including credential handling.

`dado` is intentionally I/O-free: it does not open sockets, read URLs, spawn a
runtime, or write files. Vulnerabilities in a caller's HTTP client or storage
layer are outside this repository's scope, but we will help route reports when
possible.

## Safe harbor

We will not pursue legal action against researchers who make a good-faith
effort to follow this policy, avoid privacy violations and service disruption,
and give us reasonable time to fix the issue before disclosure.
