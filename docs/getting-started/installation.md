---
title: Installation
description: Install sshnav from source or a native Linux package.
---

sshnav runs on Linux and macOS terminals. Connections use the system `ssh` and file transfers use `scp`, both provided by OpenSSH.

## Requirements

- OpenSSH client (`ssh` and `scp`).
- A terminal with interactive TTY support for the picker and forms.
- Rust only when installing from source.
- Optional: `mosh` for hosts using the mosh template.

## Install from source

```bash
git clone https://github.com/OWNER/sshnav.git
cd sshnav
cargo install --path . --locked
sshnav --version
```

`cargo install` places the binary in Cargo's binary directory, commonly `~/.cargo/bin`. Ensure that directory is in `PATH`.

## Debian package

```bash
sudo apt install ./sshnav_VERSION_amd64.deb
```

The package declares `openssh-client` as a dependency and suggests `mosh`.

## RPM package

```bash
sudo dnf install ./sshnav-VERSION-1.x86_64.rpm
```

## Verify the environment

```bash
sshnav doctor
```

The doctor checks the database location, permissions, OpenSSH programs, generated config, and private-key sources. Fix required failures before importing sensitive identities.
