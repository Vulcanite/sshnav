---
title: Interactive picker
description: Navigate sshnav's searchable terminal interface entirely from the keyboard.
---

Run `sshnav` with no command, or use `sshnav pick` with an initial filter:

```bash
sshnav
sshnav pick production
```

Search covers aliases, display names, groups, hostnames, users, and tags. Results are fuzzy-ranked and update as you type.

The details panel shows connection metadata, authentication mode, proxy jump, and a background TCP reachability summary. “Reachable” means the host accepted a TCP connection on its SSH port; it does not prove authentication will succeed.

## Edit and deletion

Press <kbd>Ctrl</kbd>+<kbd>E</kbd> to edit the selected host. Deletion is available inside the edit form and requires two consecutive <kbd>Ctrl</kbd>+<kbd>D</kbd> presses, reducing accidental removal.

## Headless use

The TUI is optional. Scripts and minimal terminals can use `sshnav host`, `sshnav connect`, `sshnav send`, and `sshnav receive` directly.
