---
title: Three-minute quickstart
description: Add your first host, open the picker, and make a connection.
---

## 1. Add a host

Use the guided terminal form:

```bash
sshnav add
```

Or add the same host non-interactively:

```bash
sshnav host add \
  --alias prod \
  --hostname 10.0.0.10 \
  --user ubuntu \
  --identity-file ~/.ssh/prod
```

The identity is validated, encrypted, and copied into sshnav's SQLite database. The source path remains metadata so `doctor` can explain where the key originated.

## 2. Open the picker

```bash
sshnav
```

Start typing to fuzzy-filter the inventory. Press <kbd>Enter</kbd> to connect.

| Key | Action |
| --- | --- |
| <kbd>↑</kbd> / <kbd>↓</kbd> | Move through results |
| <kbd>Enter</kbd> | Connect |
| <kbd>Ctrl</kbd>+<kbd>A</kbd> | Add a host |
| <kbd>Ctrl</kbd>+<kbd>E</kbd> | Edit the selected host |
| <kbd>Ctrl</kbd>+<kbd>D</kbd> | Duplicate the selected host |
| <kbd>Esc</kbd> | Exit |

## 3. Connect directly

The picker is optional. Every host is addressable by alias:

```bash
sshnav connect prod
```

sshnav constructs an argument vector and launches `ssh` directly. It never interpolates the connection into a shell command.

## 4. Transfer a file

```bash
sshnav send prod ./release.tar /srv/releases/
sshnav receive prod /var/log/app.log ./logs/
```

Add `-r` or `--recursive` for directories. Transfers reuse the host's user, hostname, port, options, jump route, and encrypted identity.
