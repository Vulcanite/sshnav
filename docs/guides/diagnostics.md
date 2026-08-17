---
title: Diagnostics and recovery
description: Use sshnav doctor, reachability checks, and bounded reconnect behavior.
---

## Environment health

```bash
sshnav doctor
```

[![sshnav doctor environment report with application paths and dependency checks](/img/screenshots/doctor-environment.png)](/img/screenshots/doctor-environment.png)

<p style={{textAlign: 'center'}}><em>A sanitized environment report with key-source metadata omitted.</em></p>

The report checks application paths, database permissions, `ssh`, optional local `rsync` availability, generated-configuration drift, and installed include state. If `sshnav.generated` no longer matches the database, regenerate it with `sshnav generate`.

## Host chain

```bash
sshnav doctor prod-api
```

[![sshnav doctor output showing two proxy hops and a target host](/img/screenshots/doctor-host-chain.png)](/img/screenshots/doctor-host-chain.png)

<p style={{textAlign: 'center'}}><em>A sanitized nested route with two proxy hops and the target.</em></p>

Saved jump aliases are recursively expanded. Each proxy and the final target receives a short TCP connection attempt. DNS failures and connection errors are displayed per hop.

## Automatic reconnect

Enable reconnect for a host:

```bash
sshnav host edit prod --auto-reconnect
```

Reconnect is deliberately narrow. sshnav retries only when SSH exits with transport status `255` after a session lasted at least 30 seconds. It makes at most three attempts with delays of 1, 2, and 4 seconds. Authentication failures and short-lived command failures are not retried.

Disable it with:

```bash
sshnav host edit prod --no-auto-reconnect
```
