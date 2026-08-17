---
title: Installation
description: Install sshnav from source or a native Linux package.
---

import CodeBlock from '@theme/CodeBlock';
import packageJson from '@site/package.json';

sshnav runs on Linux and macOS terminals. Connections use the system `ssh` and file transfers use `scp`, both provided by OpenSSH.

## Requirements

- OpenSSH client (`ssh` and `scp`).
- A terminal with interactive TTY support for the picker and forms.
- Rust only when installing from source.
- Optional: `mosh` for hosts using the mosh template.

## Install from source

```bash
git clone https://github.com/Vulcanite/sshnav.git
cd sshnav
cargo install --path . --locked
sshnav --version
```

`cargo install` places the binary in Cargo's binary directory, commonly `~/.cargo/bin`. Ensure that directory is in `PATH`.

## Debian package

Download the `.deb` from the [latest GitHub release](https://github.com/Vulcanite/sshnav/releases/latest), then install it:

<CodeBlock language="bash">{`sudo apt install ./sshnav_${packageJson.version}_amd64.deb`}</CodeBlock>

The package declares `openssh-client` as a dependency and suggests `mosh`.

## RPM package

Download the `.rpm` from the [latest GitHub release](https://github.com/Vulcanite/sshnav/releases/latest), then install it:

<CodeBlock language="bash">{`sudo dnf install ./sshnav-${packageJson.version}-1.x86_64.rpm`}</CodeBlock>

## Verify the environment

```bash
sshnav doctor
```

The doctor checks the database location, permissions, OpenSSH programs, generated config, and private-key sources. Fix required failures before importing sensitive identities.

## Shell completions

Generate completion code for your shell with `sshnav completions <SHELL>`. See the [CLI reference](../reference/cli.md#shell-completions) for examples.
