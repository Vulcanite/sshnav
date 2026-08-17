---
title: Changelog
description: User-visible changes in each sshnav release.
---

This changelog records user-visible behavior. It is kept unversioned with the rest of the documentation; each release is a section on this page.

## 0.1.3

### Changed

- `sshnav doctor` no longer reports private-key source metadata. Encrypted keys continue to use sshnav's stored copy independently of the original file.

## 0.1.2

### Added

- Optional `--rsync` transfers with archive, compression, partial-transfer, progress, delta, protected remote arguments, and the saved SSH transport settings.

## 0.1.1

### Added

- Generated shell completions for Bash, Elvish, Fish, PowerShell, and Zsh.
- A doctor check that detects stale or hand-edited generated SSH configuration.

## 0.1.0

### Added

- Native `sshnav send` and `sshnav receive` file transfers through `scp`.
- Short `-r` and long `--recursive` flags, with options shown after positional paths.
- Saved-alias proxy jump expansion for SSH, SCP, and diagnostics, including nested routes and IPv6-safe targets.
- Host duplication through `sshnav host duplicate` and <kbd>Ctrl</kbd>+<kbd>D</kbd> in the picker.
- Jump-host editing and completion in the interactive host form.
- Independent encrypted-key records for duplicated hosts.

### Changed

- Proxy jump cycles are rejected before launch.
- Mosh connections explicitly reject proxy jumps.
- Local directory sends require recursive mode.
- Remote transfer paths containing control characters are rejected.
- Imported SSH directives that can execute commands are commented out in generated configuration.

---

Future release sections can be prepared automatically by the repository's Copilot changelog workflow and reviewed in a pull request before merge.
