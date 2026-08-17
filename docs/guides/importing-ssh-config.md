---
title: Importing OpenSSH config
description: Import concrete OpenSSH Host entries and optionally generate an sshnav include file.
---

## Import

```bash
sshnav import ssh-config
sshnav import ssh-config --from ~/.ssh/work-config
```

`migrate` is retained as an equivalent migration-oriented command:

```bash
sshnav migrate --from ~/.ssh/config
```

The importer reads ordinary directives inside `Host` blocks, imports concrete host patterns, applies matching wildcard/default blocks in OpenSSH order, and preserves supported unknown directives as raw options.

Wildcard-only records are not inventory hosts. Negated patterns are respected when determining whether a default block applies.

When an imported host references an identity file, sshnav asks before copying it into encrypted storage.

## Generate an include

```bash
sshnav generate
sshnav generate --install-include
```

The first command writes the managed file. The second offers to install the corresponding `Include` line into your OpenSSH config. Add `--yes` for non-interactive confirmation.

:::info Source of truth
Generated OpenSSH config is disposable output. Edit hosts in sshnav, then regenerate it.
:::
