---
title: Contributing
description: Build, test, document, and package sshnav changes.
---

## Rust checks

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all-features --locked
cargo audit
```

## Documentation checks

```bash
npm ci
npm run typecheck
npm run docs:build
```

Use `npm run docs:start` for a local development server with live reload.

## Writing guidelines

- Prefer runnable commands over abstract descriptions.
- State defaults and failure behavior explicitly.
- Keep security boundaries near the feature they constrain.
- Update the [CLI reference](./reference/cli.md) when command flags change.
- Update the [changelog](./changelog.md) for user-visible releases.

## Changelog automation

Publishing a GitHub Release triggers the Copilot changelog workflow. Copilot reads the repository history, updates only `docs/changelog.md`, and the workflow opens a pull request for human review. The repository or organization must permit Copilot CLI requests from Actions.

## Packaging

```bash
cargo build --release --locked
packaging/deb/build-deb.sh
packaging/rpm/build-rpm.sh
```

Generated packages are written to `dist/` and are not committed.
