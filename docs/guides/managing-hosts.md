---
title: Managing hosts
description: Add, edit, list, duplicate, and remove sshnav hosts.
---

## Add

```bash
sshnav host add \
  --alias prod-api \
  --hostname api.example.com \
  --user deploy \
  --port 2222 \
  --group production \
  --tag api --tag critical
```

Use `sshnav add` when you prefer a guided form with group, jump-host, and private-key completion.

[![sshnav guided add-host form with a populated example host](/img/screenshots/add-host-form.png)](/img/screenshots/add-host-form.png)

<p style={{textAlign: 'center'}}><em>The guided form previews the saved connection while you enter it.</em></p>

## List

```bash
sshnav host list
sshnav host list --json
```

JSON output is useful for inspection and small local scripts. Secret material is never included.

## Edit

```bash
sshnav host edit prod-api --hostname api-2.example.com --port 22
sshnav host edit prod-api --proxy-jump bastion
sshnav host edit prod-api --no-proxy-jump
```

Only supplied fields change. Use the explicit `--no-*` flags to disable reconnect or clear a jump route.

## Duplicate

```bash
sshnav host duplicate prod-api staging-api
```

Every saved host field is copied. If the source has an encrypted key, the destination receives an independent database secret record, so later deleting either host does not remove the other's key.

In the picker, <kbd>Ctrl</kbd>+<kbd>D</kbd> opens a prefilled duplicate form so you can adjust the hostname, user, port, tags, group, jump route, and authentication before saving.

## Remove

```bash
sshnav host remove staging-api
```

Removing a host also removes its associated tags, forwards, options, and encrypted secret rows.
