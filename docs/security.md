---
title: Security model
description: Understand sshnav's threat model, host-key behavior, key encryption, and command boundaries.
---

sshnav is a local, single-user SSH inventory and launcher. It assumes the operating-system account running it is trusted.

## Private-key storage

- Imported private-key bytes are encrypted before entering SQLite.
- XChaCha20-Poly1305 provides authenticated encryption.
- Argon2id derives the encryption key using 64 MiB and three iterations.
- A random owner-only local `secret.key` supplies the passphrase material.
- Decrypted temporary identities use owner-only permissions and live only for the child process.

## Command execution

SSH and SCP argument vectors are passed directly to `Command`; they are never joined into shell strings. Hostnames, aliases, and remote transfer paths are validated, and remote paths containing control characters are rejected.

Only a constrained allowlist of imported OpenSSH options is forwarded to direct SSH and SCP commands. sshnav supplies strict host-key settings and uses the user's normal `known_hosts` file.

## What encryption does not protect

An attacker controlling the same OS user can read the local encryption key, database, process environment, and temporary files during active connections. sshnav is not a general-purpose secrets manager and has not been independently audited.

## Passwords

sshnav does not store SSH passwords. Password authentication remains an OpenSSH prompt.

## Reporting vulnerabilities

Use the repository's private GitHub security advisory feature. Do not open a public issue when a report could expose identities, encryption material, or command-execution behavior.
