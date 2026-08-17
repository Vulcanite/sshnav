---
title: Paths and environment
description: Locate sshnav's database, encryption key, runtime files, and generated OpenSSH config.
---

sshnav follows platform data and runtime conventions. On a typical Linux system:

| Data | Typical path |
| --- | --- |
| SQLite inventory | `~/.local/share/sshnav/sshnav.db` |
| Local encryption key | `~/.local/share/sshnav/secret.key` |
| Temporary identities | `$XDG_RUNTIME_DIR/sshnav/` |
| Generated SSH config | `~/.ssh/sshnav.generated` |
| OpenSSH config | `~/.ssh/config` |

Set `SSHNAV_DATA_DIR` to relocate persistent sshnav data:

```bash
export SSHNAV_DATA_DIR=/secure/local/sshnav
```

The database and local encryption key are owner-only files. Runtime directories are owner-only, and decrypted identity files use mode `0600` on Unix.

Keep the database and `secret.key` together in backups. Losing the local key makes encrypted identity copies unrecoverable.
