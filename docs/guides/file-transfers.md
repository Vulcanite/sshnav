---
title: File transfers
description: Send and receive files through native SCP using saved host settings.
---

sshnav launches `scp` directly and reuses the selected host's hostname, user, port, jump chain, safe options, host-key policy, and encrypted identity lifecycle.

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

A local directory source is rejected without the recursive flag. Missing local sources and control characters in remote paths are rejected before launching SCP.

## Native SCP behavior

Destinations follow normal SCP semantics: they can be existing directories or renamed file paths. Existing files follow SCP's overwrite behavior. Transfers run once without application-level retry or resume.
