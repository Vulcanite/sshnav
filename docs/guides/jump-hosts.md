---
title: Jump hosts
description: Connect and transfer files through literal or saved ProxyJump routes.
---

Set a saved alias as the jump host:

```bash
sshnav host add --alias bastion --hostname 203.0.113.10 --user jump
sshnav host edit prod --proxy-jump bastion
sshnav connect prod
```

sshnav expands the alias to its saved user, hostname, and non-default port before passing `-J` to SSH or SCP. IPv6 hostnames are bracketed safely.

## Nested routes

A jump host may itself reference another saved alias:

```text
prod → regional-bastion → edge-gateway
```

sshnav recursively expands the route in connection order. Literal OpenSSH values can be mixed with saved aliases:

```bash
sshnav host edit prod --proxy-jump 'ops@edge.example:2200,regional-bastion'
```

Cycles are rejected before a process is launched.

## Authentication boundary

The target may use an encrypted identity stored by sshnav. Intermediate jump hosts authenticate through OpenSSH's normal agent and configuration. Configure jump-specific identities in `~/.ssh/config` or load them into an agent.

## Diagnose the chain

```bash
sshnav doctor prod
```

The output lists every expanded proxy followed by the target, with hostname, port, user, and TCP reachability.

:::warning Mosh
Mosh hosts cannot use proxy jumps. Switch the host to the normal SSH template when a jump route is required.
:::
