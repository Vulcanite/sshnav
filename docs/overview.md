---
title: sshnav documentation
description: Learn how to organize, connect to, diagnose, and transfer files between SSH hosts with sshnav.
slug: /
---

import packageJson from '@site/package.json';

<pre className="ascii-doc" aria-label="sshnav">
{`███████ ██████  ██   ██ ███    ██  █████  ██    ██
██      ██      ██   ██ ████   ██ ██   ██ ██    ██
███████ ██████  ███████ ██ ██  ██ ███████ ██    ██
     ██      ██ ██   ██ ██  ██ ██ ██   ██  ██  ██
███████ ██████  ██   ██ ██   ████ ██   ██   ████`}
</pre>

Current release: <code>v{packageJson.version}</code>

sshnav is a local SSH inventory, searchable terminal picker, connection launcher, and file-transfer frontend. It stores the details that are easy to forget—aliases, users, ports, jump routes, tags, forwards, options, and identity metadata—then launches native `ssh`, `scp`, or optional `rsync` directly.

## What sshnav owns

- A SQLite inventory of hosts and templates.
- Encrypted copies of imported private keys.
- Optional generated OpenSSH configuration.
- Interactive add, edit, duplicate, and picker experiences.

## What OpenSSH owns

- The SSH protocol and authentication exchange.
- Host-key verification and `known_hosts`.
- Password prompts, SSH agents, and agent forwarding.
- Network transport for SSH, SCP, and rsync.

:::tip Start here
Install sshnav, add one host, and open the picker with the [three-minute quickstart](./getting-started/quickstart.md).
:::

## Core workflows

| Goal | Command |
| --- | --- |
| Open the picker | `sshnav` |
| Connect by alias | `sshnav connect prod` |
| Copy a file to a host | `sshnav send prod ./release.tar .` |
| Copy logs from a host | `sshnav receive prod /var/log/app.log .` |
| Inspect a jump chain | `sshnav doctor prod` |
| Import OpenSSH hosts | `sshnav import ssh-config` |

Continue with [core concepts](./getting-started/core-concepts.md), or jump directly to the [CLI reference](./reference/cli.md).
