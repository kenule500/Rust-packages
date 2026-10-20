## Summary

<!-- What changed, and why? Keep it focused. -->

## Linked work

<!-- A SpecDrive feature or backlog item, for example BP-001 or m0-foundation. -->

- Backlog / spec:
- Task IDs:

## Test evidence

<!--
Paste the commands you ran and their results. Tests are authored and executed
by /sdrive:test for SpecDrive features; link that evidence when applicable.
-->

```text
cargo fmt --all --check
cargo clippy --workspace --all-features -- -D warnings
cargo test --workspace --all-features
```

## Checklist

- [ ] The change is linked to a backlog item or SpecDrive feature.
- [ ] The branch is named `feature/<name>` and targets the agreed base.
- [ ] Formatting passes.
- [ ] Clippy passes with warnings denied.
- [ ] Tests pass, or the reason they are not applicable is explained above.
- [ ] Documentation builds with warnings denied.
- [ ] Packaging succeeds for metadata or release changes.
- [ ] New public items have rustdoc.
- [ ] No secrets, tokens, credentials, or generated build output are committed.
- [ ] `CHANGELOG.md` is updated for user-visible changes.
- [ ] The change stays inside the approved task scope.
- [ ] Security-sensitive changes follow `SECURITY.md`.
