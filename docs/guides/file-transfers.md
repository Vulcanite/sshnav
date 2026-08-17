---
title: File transfers
description: Send and receive files through native SCP or rsync using saved host settings.
---

sshnav uses `scp` by default and reuses the selected host's hostname, user, port, jump chain, safe options, host-key policy, and encrypted identity lifecycle.

## Send

```bash
sshnav send prod ./release.tar /srv/releases/
```

The remote destination defaults to `.`, the remote home directory:

```bash
sshnav send prod ./release.tar
```

## Receive

```bash
sshnav receive prod /var/log/app.log ./logs/
```

The local destination defaults to `.`, the current directory:

```bash
sshnav receive prod /var/log/app.log
```

## Directories

Add either recursive spelling after the paths:

```bash
sshnav send prod ./public /srv/www -r
sshnav receive prod /var/log/myapp ./logs --recursive
```

A local directory source is rejected without the recursive flag. Missing local sources and control characters in remote paths are rejected before launching either backend.

## Resumable rsync transfers

Add `--rsync` after the paths to use archive mode with compression, partial-transfer retention, progress output, and delta transfer:

```bash
sshnav send prod ./public /srv/www -r --rsync
sshnav receive prod /var/log/myapp ./logs -r --rsync
```

rsync must be installed on both the local and remote machines. sshnav passes protected path arguments and constructs rsync's SSH transport from the same saved identity, port, jump route, and host-key settings used by SCP.

## Native transfer behavior

Destinations follow the selected backend's native semantics: they can be existing directories or renamed file paths. SCP transfers run once without application-level retry or resume; rsync provides its native partial and delta-transfer behavior.
