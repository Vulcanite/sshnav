<p align="center">
  <img src="static/img/ascii-wordmark.svg" alt="sshnav" width="640">
</p>

# sshnav

`sshnav` is a fast local SSH inventory navigator and launcher for Linux and macOS terminals.

## Quickstart

Build or install from source:

```sh
cargo install --path .
```

Add a host:

```sh
sshnav host add --alias prod --hostname 10.0.0.10 --user ubuntu --identity-file ~/.ssh/prod
```

Open the navigator and connect:

```sh
sshnav
```

For a guided terminal form instead of flags:

```sh
sshnav add
```

## Features

- SQLite-backed host inventory with aliases, groups, tags, users, ports, forwards, and OpenSSH options.
- Compact TUI navigator with fuzzy filtering, reachability status, host details, `Ctrl-A` add, `Ctrl-E` edit, `Ctrl-D` duplicate, and guarded delete from edit mode.
- OpenSSH config import and migration, including matching wildcard/default `Host` block inheritance such as `Host *`.
- Optional generated OpenSSH include file via `sshnav generate`.
- Encrypted imported private-key storage using an owner-only local key file, suitable for headless terminals.
- Host chain diagnostics via `sshnav doctor <alias>`, including proxy jump hops.
- Direct launch via OpenSSH or mosh-style templates using argv construction, not shell strings.
- Native `scp` send and receive commands, with optional resumable `rsync`, using each host's saved SSH settings and authentication.
- SSH keepalives, strict host-key checking, and an opt-in per-host reconnect policy (three bounded retries after a live session drops).
- `sshnav doctor` health checks for paths, OpenSSH availability, generated config/include status, and file permissions.

Useful commands:

```sh
sshnav pick [query]
sshnav connect <alias>
sshnav send <alias> <local-source> [remote-destination] [-r|--recursive] [--rsync]
sshnav receive <alias> <remote-source> [local-destination] [-r|--recursive] [--rsync]
sshnav host list [--json]
sshnav host edit <alias> [flags...]
sshnav host edit <alias> --proxy-jump <saved-alias>
sshnav host edit <alias> --no-proxy-jump
sshnav host duplicate <source-alias> <new-alias>
sshnav host update-key <alias> --from ~/.ssh/key
sshnav host remove-key <alias>
sshnav host forget-key-source <alias>
sshnav host remove <alias>
sshnav import ssh-config [--from ~/.ssh/config]
sshnav migrate [--from ~/.ssh/config]
sshnav generate [--install-include] [--yes]
sshnav doctor [alias]
```

`send` defaults to the remote home directory (`.`), while `receive` defaults to the current local directory (`.`). Add `-r` or `--recursive` when transferring a directory. SCP is the default backend; add `--rsync` for native archive, compression, partial-transfer, progress, and delta behavior when rsync is installed on both machines.

`--proxy-jump` accepts either a normal OpenSSH `ProxyJump` value or a saved sshnav alias. Saved aliases are expanded to their hostname, user, and port for SSH, SCP, and rsync; nested saved aliases are expanded in order. Jump hosts use OpenSSH's normal authentication (agent or SSH config). Mosh connections do not support proxy jumps.

`host duplicate` copies every saved host setting and gives the copy its own encrypted-key record. In the picker, select a host and press `Ctrl-D` to open a prefilled duplicate form before saving.

## Non-goals

- Not a general-purpose secrets manager.
- Not independently security-audited.
- Assumes a single-user, single-machine trust boundary. OS-keyring-backed encryption does not protect against an attacker who already controls the same OS user account. See [SECURITY.md](SECURITY.md).
- Does not store SSH passwords. Password auth goes through OpenSSH's normal interactive prompt.

## No Telemetry

`sshnav` makes no network calls except the SSH, SCP, rsync, or mosh processes it launches on your behalf.

## Security

Imported private keys are encrypted before being stored in SQLite. The encryption key is kept in an owner-only local `secret.key` file, avoiding desktop-keyring dependencies in headless terminals. See [SECURITY.md](SECURITY.md) for the threat model and vulnerability reporting process.

## Linux Packages

Build local `.deb` and `.rpm` packages after compiling the release binary:

```sh
cargo build --release --locked
packaging/deb/build-deb.sh
packaging/rpm/build-rpm.sh
```

Packages are written to `dist/`. The release workflow builds the same packages on Ubuntu and uploads them as workflow artifacts. On `v*` tags, it also creates or updates the matching GitHub Release and uploads the `.deb` and `.rpm`.

## Documentation

The Docusaurus website lives in `docs/` with its theme and landing page under `src/`. Preview it locally with:

```sh
npm ci
npm run docs:start
```

Pull requests verify the production build. Pushes to `main` or `master` deploy it through GitHub Pages after Pages is configured to use **GitHub Actions** as its source.

The release-triggered changelog workflow requires a fine-grained personal access token with the **Copilot Requests** permission stored as the `COPILOT_GITHUB_TOKEN` repository secret.

## License

Licensed under the terms in [LICENSE](LICENSE).

## AI-assisted development

This project is developed with assistance from [GPT-5.6 Sol](https://developers.openai.com/api/docs/models/gpt-5.6-sol), using OpenAI Codex as the coding harness. Codex assists with implementation, testing, review, documentation, and release engineering; project direction, final decisions, and responsibility remain with the maintainer.
