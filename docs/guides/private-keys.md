---
title: Private keys
description: Import, replace, remove, and understand sshnav's encrypted private-key copies.
---

## Import on add

```bash
sshnav host add \
  --alias prod \
  --hostname prod.example.com \
  --user ubuntu \
  --identity-file ~/.ssh/prod
```

sshnav validates the file, encrypts its bytes with XChaCha20-Poly1305, and stores the encrypted blob in SQLite. Key derivation uses Argon2id and a random owner-only local secret.

## Replace a key

```bash
sshnav host update-key prod --from ~/.ssh/prod-rotated
```

## Remove a key

```bash
sshnav host remove-key prod
```

This clears both the encrypted copy and source metadata. To retain the encrypted copy but remove only its fallback source path:

```bash
sshnav host forget-key-source prod
```

The latter refuses to run if no encrypted copy exists.

## Process lifetime

For SSH or SCP, sshnav decrypts the stored identity into a `0600` temporary file inside its private runtime directory. The temporary file object remains alive until the child process exits and is then deleted.

:::danger Trust boundary
Encryption protects against some disk, backup, and cross-account exposure. It cannot protect keys from an attacker who already controls the same operating-system user and can read both the database and local encryption key.
:::
