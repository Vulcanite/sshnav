---
title: Core concepts
description: Understand inventory records, aliases, authentication, templates, and generated configuration.
---

## Inventory

SQLite is sshnav's source of truth. A host record can contain:

- alias, display name, group, and tags;
- hostname, user, and port;
- proxy jump route;
- local, remote, and dynamic forwards;
- safe OpenSSH options;
- template and reconnect policy;
- encrypted private-key presence and optional source metadata.

## Aliases

An alias is the stable name used by every command. Prefer short, meaningful values such as `prod-api`, `staging-db`, or `home-lab`.

```bash
sshnav connect prod-api
sshnav doctor prod-api
sshnav send prod-api ./release .
```

## Authentication

With **OpenSSH default**, sshnav supplies no identity and lets OpenSSH use its config, agent, and prompts. With **private key**, sshnav encrypts a copy locally and decrypts it into an owner-only temporary file only for the child process lifetime.

Jump hosts use OpenSSH's normal agent or SSH-config authentication. sshnav's encrypted target identity is not automatically assigned to intermediate jump hosts.

## Templates

The normal path launches OpenSSH. A host may instead use the `mosh` template. Because mosh does not implement OpenSSH `ProxyJump`, sshnav rejects mosh hosts configured with a jump route.

## Generated OpenSSH config

SQLite remains authoritative. `sshnav generate` writes a disposable OpenSSH include file for tools that need ordinary `Host` entries. Encrypted identities are intentionally not exposed in generated config because OpenSSH cannot read sshnav's encrypted database.
