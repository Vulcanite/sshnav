---
title: CLI reference
description: Complete sshnav command, argument, option, default, and exit-behavior reference.
---

```text
sshnav [COMMAND]
```

With no command, sshnav opens the interactive picker.

## Navigation and connections

### `sshnav pick [QUERY]`

Open the picker with an optional initial fuzzy-search query.

### `sshnav connect <ALIAS>`

Connect directly to a saved alias. Unknown aliases return a suggestion when a close match exists.

## File transfers

```text
sshnav send <ALIAS> <LOCAL_SOURCE> [REMOTE_DESTINATION] [OPTIONS]
sshnav receive <ALIAS> <REMOTE_SOURCE> [LOCAL_DESTINATION] [OPTIONS]
```

| Argument | Meaning | Default |
| --- | --- | --- |
| `ALIAS` | Saved sshnav host | required |
| `LOCAL_SOURCE` | Local file or directory to send | required |
| `REMOTE_SOURCE` | Remote file or directory to receive | required |
| `REMOTE_DESTINATION` | Remote directory or renamed path | `.` |
| `LOCAL_DESTINATION` | Local directory or renamed path | `.` |

`-r, --recursive` enables directory copying. `--rsync` switches from the default `scp` backend to `rsync -avzP`, with protected remote arguments; rsync must be installed locally and remotely.

## Host management

### `sshnav host list [--json]`

List saved hosts as aligned text or JSON. Secret bytes are never returned.

### `sshnav host add`

```text
sshnav host add --alias <ALIAS> --hostname <HOSTNAME> --user <USER> [OPTIONS]
```

| Option | Meaning |
| --- | --- |
| `--port <PORT>` | Non-default SSH port |
| `--tag <TAG>` | Tag; repeat or pass comma-separated values |
| `--display-name <NAME>` | Longer human-readable name |
| `--group <GROUP>` | Picker grouping value |
| `--identity-file <PATH>` | Validate, encrypt, and import a private key |
| `--template <NAME>` | Connection template, such as `mosh` |
| `--proxy-jump <VALUE>` | Saved alias or literal OpenSSH ProxyJump route |
| `--auto-reconnect` | Enable bounded reconnect after live transport loss |

### `sshnav host edit <ALIAS> [OPTIONS]`

Uses the same mutable fields as `host add`. Additional flags:

| Option | Meaning |
| --- | --- |
| `--no-proxy-jump` | Clear the saved jump route |
| `--no-auto-reconnect` | Disable reconnect |

### `sshnav host duplicate <SOURCE_ALIAS> <NEW_ALIAS>`

Copy every saved host field and create an independent encrypted-key record.

### `sshnav host remove <ALIAS>`

Delete a host and its associated child records and secrets.

## Key management

```text
sshnav host update-key <ALIAS> --from <PATH>
sshnav host remove-key <ALIAS>
sshnav host forget-key-source <ALIAS>
```

`update-key` validates and replaces the encrypted key. `remove-key` removes the encrypted copy and source metadata. `forget-key-source` retains the encrypted copy while removing only its source path.

## OpenSSH interoperability

```text
sshnav import ssh-config [--from <PATH>]
sshnav migrate [--from <PATH>]
sshnav generate [--install-include] [--yes]
```

Import and migrate default to `~/.ssh/config`. Generate writes the managed include; `--install-include` offers to add it to the user's config and `--yes` accepts the prompt.

## Diagnostics

```text
sshnav doctor [ALIAS]
```

Without an alias, check local environment health. With an alias, resolve and probe its jump chain and target.

## Shell completions

```text
sshnav completions <SHELL>
```

Generate completion code for Bash, Elvish, Fish, PowerShell, or Zsh. For example:

```bash
sshnav completions bash > ~/.local/share/bash-completion/completions/sshnav
sshnav completions fish > ~/.config/fish/completions/sshnav.fish
sshnav completions zsh > ~/.zfunc/_sshnav
```

## Global options

| Option | Meaning |
| --- | --- |
| `-h, --help` | Print contextual command help |
| `-V, --version` | Print the sshnav version |

## Exit behavior

- Parse errors return `2`.
- Local validation and application errors are reported without invoking SSH, SCP, or rsync.
- Connections and transfers propagate the native child's exit status where available.
- A child terminated without an exit code maps to `1`.
