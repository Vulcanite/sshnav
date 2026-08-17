use crate::storage::SecretBlob;
use anyhow::{Context, Result, anyhow, bail};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use rand_core::{OsRng, RngCore};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::NamedTempFile;
use zeroize::Zeroize;

const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 24;
const KEY_LEN: usize = 32;
const LOCAL_KEY_LEN: usize = 32;

pub fn encrypt_bytes(
    plaintext: &[u8],
    passphrase: &str,
    source_path: Option<String>,
) -> Result<SecretBlob> {
    let mut salt = [0_u8; SALT_LEN];
    let mut nonce = [0_u8; NONCE_LEN];
    OsRng.fill_bytes(&mut salt);
    OsRng.fill_bytes(&mut nonce);

    let mut key_bytes = derive_key(passphrase, &salt)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key_bytes));
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), plaintext)
        .map_err(|_| anyhow!("could not encrypt secret"))?;
    key_bytes.zeroize();

    Ok(SecretBlob {
        salt: salt.to_vec(),
        nonce: nonce.to_vec(),
        ciphertext,
        source_path,
    })
}

pub fn decrypt_bytes(blob: &SecretBlob, passphrase: &str) -> Result<Vec<u8>> {
    if blob.salt.len() != SALT_LEN || blob.nonce.len() != NONCE_LEN {
        bail!("stored secret has invalid encryption metadata");
    }
    let mut key_bytes = derive_key(passphrase, &blob.salt)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key_bytes));
    let plaintext = cipher
        .decrypt(XNonce::from_slice(&blob.nonce), blob.ciphertext.as_ref())
        .map_err(|_| anyhow!("could not decrypt secret; wrong passphrase or corrupted data"))?;
    key_bytes.zeroize();
    Ok(plaintext)
}

pub fn local_key_passphrase(path: &Path) -> Result<String> {
    if let Some(secret) = read_legacy_local_key(path)? {
        return Ok(secret);
    }
    let mut key = [0_u8; LOCAL_KEY_LEN];
    OsRng.fill_bytes(&mut key);
    write_local_key_file(path, &key)?;
    let passphrase = to_hex(&key);
    key.zeroize();
    Ok(passphrase)
}

pub fn encrypt_file(path: &Path, passphrase: &str) -> Result<SecretBlob> {
    let resolved = resolve_private_key_path(path)?;
    validate_resolved_private_key(&resolved)?;
    let bytes = fs::read(&resolved)
        .with_context(|| format!("could not read private key at {}", resolved.display()))?;
    encrypt_bytes(&bytes, passphrase, Some(resolved.display().to_string()))
}

pub fn validate_private_key(path: &Path) -> Result<()> {
    let resolved = resolve_private_key_path(path)?;
    validate_resolved_private_key(&resolved)
}

pub fn resolve_private_key_path(path: &Path) -> Result<PathBuf> {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    resolve_private_key_path_with_home(path, home.as_deref())
}

fn resolve_private_key_path_with_home(path: &Path, home: Option<&Path>) -> Result<PathBuf> {
    let expanded = expand_tilde(path);
    if expanded.exists() {
        return canonical_private_key_path(&expanded);
    }

    if path.is_relative()
        && let Some(home) = home
    {
        let home_path = home.join(path);
        if home_path.exists() {
            return canonical_private_key_path(&home_path);
        }

        if path.components().count() == 1 {
            let ssh_path = home.join(".ssh").join(path);
            if ssh_path.exists() {
                return canonical_private_key_path(&ssh_path);
            }
        }
    }

    bail!(
        "private key not found at {}. Use an absolute path, ~/path, or place the key under ~/.ssh",
        expanded.display()
    );
}

fn canonical_private_key_path(path: &Path) -> Result<PathBuf> {
    if !path.is_file() {
        bail!("private key path {} is not a file", path.display());
    }
    path.canonicalize()
        .with_context(|| format!("could not resolve private key path {}", path.display()))
}

fn validate_resolved_private_key(expanded: &Path) -> Result<()> {
    let output = Command::new("ssh-keygen")
        .arg("-y")
        .arg("-f")
        .arg(expanded)
        .output()
        .with_context(|| "could not run ssh-keygen to validate private key")?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "private key at {} is not valid or requires a passphrase unsupported by sshnav{}",
            expanded.display(),
            if stderr.trim().is_empty() {
                String::new()
            } else {
                format!(": {}", stderr.trim())
            }
        );
    }
    Ok(())
}

pub fn write_temp_private_key(bytes: &[u8], runtime_dir: &Path) -> Result<NamedTempFile> {
    prepare_private_runtime_dir(runtime_dir)?;
    let mut file = NamedTempFile::new_in(runtime_dir).with_context(|| {
        format!(
            "could not create temporary private key in {}",
            runtime_dir.display()
        )
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o600))?;
    }
    file.write_all(bytes)?;
    file.flush()?;
    Ok(file)
}

fn read_legacy_local_key(path: &Path) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }
    let key = fs::read(path).with_context(|| format!("could not read {}", path.display()))?;
    if key.len() != LOCAL_KEY_LEN {
        bail!("local sshnav key at {} is invalid", path.display());
    }
    set_owner_only_permissions(path)?;
    Ok(Some(to_hex(&key)))
}

fn write_local_key_file(path: &Path, key: &[u8; LOCAL_KEY_LEN]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
        set_owner_only_dir_permissions(parent)?;
    }
    fs::write(path, key).with_context(|| format!("could not write {}", path.display()))?;
    set_owner_only_permissions(path)
}

fn prepare_private_runtime_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path).with_context(|| format!("could not create {}", path.display()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).with_context(|| {
            format!("could not set owner-only permissions on {}", path.display())
        })?;
    }
    Ok(())
}

pub fn expand_tilde(path: &Path) -> PathBuf {
    let Some(raw) = path.to_str() else {
        return path.to_path_buf();
    };
    if raw == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home);
        }
    } else if let Some(rest) = raw.strip_prefix("~/")
        && let Ok(home) = std::env::var("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    path.to_path_buf()
}

fn derive_key(passphrase: &str, salt: &[u8]) -> Result<[u8; KEY_LEN]> {
    let mut key = [0_u8; KEY_LEN];
    let params = Params::new(64 * 1024, 3, 1, Some(KEY_LEN))
        .map_err(|err| anyhow!("could not configure Argon2id: {err}"))?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(passphrase.as_bytes(), salt, &mut key)
        .map_err(|err| anyhow!("could not derive encryption key: {err}"))?;
    Ok(key)
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("could not set owner-only permissions on {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_owner_only_dir_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o700))
        .with_context(|| format!("could not set owner-only permissions on {}", path.display()))?;
    Ok(())
}

#[cfg(not(unix))]
fn set_owner_only_dir_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decrypts_with_correct_passphrase() {
        let blob = encrypt_bytes(b"secret", "passphrase", None).unwrap();
        assert_eq!(decrypt_bytes(&blob, "passphrase").unwrap(), b"secret");
    }

    #[test]
    fn rejects_wrong_passphrase() {
        let blob = encrypt_bytes(b"secret", "passphrase", None).unwrap();
        assert!(decrypt_bytes(&blob, "wrong").is_err());
    }

    #[test]
    fn reads_legacy_local_key_file_as_hex() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.key");
        fs::write(&path, [7_u8; LOCAL_KEY_LEN]).unwrap();

        let secret = read_legacy_local_key(&path).unwrap().unwrap();

        assert_eq!(secret, "07".repeat(LOCAL_KEY_LEN));
    }

    #[test]
    fn creates_an_owner_only_local_key_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("data").join("secret.key");

        let secret = local_key_passphrase(&path).unwrap();

        assert_eq!(secret.len(), LOCAL_KEY_LEN * 2);
        assert_eq!(fs::read(&path).unwrap().len(), LOCAL_KEY_LEN);
    }

    #[test]
    fn writes_private_key_temp_file_inside_runtime_dir() {
        let dir = tempfile::tempdir().unwrap();
        let runtime_dir = dir.path().join("runtime");

        let file = write_temp_private_key(b"secret-key", &runtime_dir).unwrap();

        assert!(file.path().starts_with(&runtime_dir));
        assert_eq!(fs::read(file.path()).unwrap(), b"secret-key");
    }

    #[test]
    fn resolves_bare_private_key_name_from_home() {
        let dir = tempfile::tempdir().unwrap();
        let key = dir.path().join("oracle-ind-developer");
        fs::write(&key, b"not validated here").unwrap();

        let resolved =
            resolve_private_key_path_with_home(Path::new("oracle-ind-developer"), Some(dir.path()))
                .unwrap();

        assert_eq!(resolved, key.canonicalize().unwrap());
    }

    #[test]
    fn reports_missing_private_key_before_validation() {
        let dir = tempfile::tempdir().unwrap();
        let err = resolve_private_key_path_with_home(Path::new("missing-key"), Some(dir.path()))
            .unwrap_err();

        assert!(err.to_string().contains("private key not found"));
    }
}
