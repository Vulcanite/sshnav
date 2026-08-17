# Architecture Notes

`sshnav` is split into narrow modules:

- `cli`: `clap` command definitions, typo tips, and command orchestration.
- `storage`: SQLite schema migrations, host/template persistence, and encrypted secret blobs.
- `inventory`: shared host/template data model plus validation.
- `secrets`: owner-only local-key management, Argon2id key derivation, XChaCha20-Poly1305 encryption, and temporary private-key files.
- `ssh_config`: conservative importer for OpenSSH `Host` blocks with wildcard default merging.
- `generator`: optional OpenSSH include renderer and include installer.
- `runner`: direct SSH argv construction and child process execution.
- `diagnostics`: proxy jump chain expansion and hop-by-hop TCP diagnostics.
- `picker`: compact `ratatui` searchable picker with branded empty state.
- `doctor`: local environment, DB permission, include, and private-key source checks.

SQLite is the source of truth. `~/.ssh/sshnav.generated` is disposable optional
output for users who want OpenSSH config interop.

Secrets are encrypted before they enter SQLite using XChaCha20-Poly1305 and an
Argon2id-derived key (64 MiB, three iterations). The local encryption key is a
random owner-only `secret.key` beside the database, so headless Linux terminals
do not depend on a desktop secret-service daemon. sshnav does not store SSH
passwords; OpenSSH remains responsible for password prompts when users choose
password auth outside sshnav.

Private keys are copied into encrypted DB storage when added or updated. The
original path is stored only as optional source metadata and fallback context.
During connect, a copied key is decrypted into a temporary `0600` file inside
sshnav's private runtime directory, typically `$XDG_RUNTIME_DIR/sshnav`. The
file remains alive only while the child SSH process is running.

Generated OpenSSH config does not emit the stored source path for hosts with an
encrypted private key, because OpenSSH cannot read sshnav's encrypted key store.
For path-only hosts without an encrypted copy, generation can still emit
`IdentityFile` as fallback interop.

The picker shows authentication mode and a cached TCP reachability check for the
selected host. Reachability checks run in background threads so DNS or network
timeouts cannot block the TUI draw loop. Reachability means `host:port` accepted
a TCP connection; it is not an SSH login check.

The importer is intentionally not a full OpenSSH parser. It reads ordinary
directive lines inside `Host` blocks, imports concrete patterns, applies matching
wildcard/default blocks in OpenSSH order, captures common fields, and preserves
unknown directives as raw option lines for generation/direct SSH args.
