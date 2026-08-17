# Security Policy

## Threat Model

`sshnav` is a local, single-user SSH inventory and launcher. It assumes the
operating system user account running sshnav is trusted.

The encrypted private-key vault is intended to
reduce exposure from disk image theft, backup exposure, and casual filesystem
browsing by another local account when filesystem permissions are enforced
correctly. The SQLite database and local encryption key are set to owner-only
`0600`, and sshnav's private runtime/key directories are set to owner-only
`0700` on Unix platforms.

The vault does not protect against an attacker who already has shell access as
the same OS user. That attacker can read the local encryption key.

## Local Encryption Key

sshnav stores a random local encryption key at:

```text
~/.local/share/sshnav/secret.key
```

or, when `SSHNAV_DATA_DIR` is set, at `$SSHNAV_DATA_DIR/secret.key`. The file is
created with owner-only permissions. Keep it with the database: losing it means
encrypted private-key copies cannot be decrypted.

```text
$SSHNAV_DATA_DIR/secret.key
```

## Reporting a Vulnerability

Use GitHub's private security advisory feature for the repository once the
project is published. Please do not open public issues for vulnerabilities that
expose private keys, local encryption keys, or command execution behavior.
