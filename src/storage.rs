use crate::inventory::{Host, Inventory, Template};
use crate::paths::AppPaths;
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub const SECRET_PRIVATE_KEY: &str = "private_key";
const UNSUPPORTED_SECRET_PASSWORD: &str = "password";
const CURRENT_SCHEMA_VERSION: i64 = 3;

pub struct Store {
    conn: Connection,
}

pub struct SecretBlob {
    pub salt: Vec<u8>,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
    pub source_path: Option<String>,
}

impl Store {
    pub fn open(paths: &AppPaths) -> Result<Self> {
        if let Some(parent) = paths.db.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        let conn = Connection::open(&paths.db)
            .with_context(|| format!("could not open database at {}", paths.db.display()))?;
        set_owner_only_permissions(&paths.db)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let store = Self { conn };
        store.migrate_schema()?;
        Ok(store)
    }

    pub fn migrate_schema(&self) -> Result<()> {
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version > CURRENT_SCHEMA_VERSION {
            bail!("database schema version {version} is newer than this sshnav build");
        }
        if version == 0 {
            self.conn.execute_batch(
                r#"
CREATE TABLE IF NOT EXISTS templates (
    name TEXT PRIMARY KEY NOT NULL,
    argv_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS hosts (
    alias TEXT PRIMARY KEY NOT NULL,
    display_name TEXT,
    group_name TEXT,
    hostname TEXT NOT NULL,
    user TEXT,
    port INTEGER,
    identity_file TEXT,
    template TEXT,
    proxy_jump TEXT
);

CREATE TABLE IF NOT EXISTS host_tags (
    host_alias TEXT NOT NULL REFERENCES hosts(alias) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    tag TEXT NOT NULL,
    PRIMARY KEY (host_alias, position)
);

CREATE TABLE IF NOT EXISTS host_forwards (
    host_alias TEXT NOT NULL REFERENCES hosts(alias) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    position INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (host_alias, kind, position)
);

CREATE TABLE IF NOT EXISTS host_options (
    host_alias TEXT NOT NULL REFERENCES hosts(alias) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    value TEXT NOT NULL,
    PRIMARY KEY (host_alias, position)
);

CREATE TABLE IF NOT EXISTS secrets (
    host_alias TEXT NOT NULL REFERENCES hosts(alias) ON DELETE CASCADE,
    kind TEXT NOT NULL,
    source_path TEXT,
    salt BLOB NOT NULL,
    nonce BLOB NOT NULL,
    ciphertext BLOB NOT NULL,
    PRIMARY KEY (host_alias, kind)
);

PRAGMA user_version = 1;
"#,
            )?;
        }
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < 2 {
            self.migrate_schema_v2()?;
        }
        let version: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        if version < 3 {
            self.migrate_schema_v3()?;
        }
        self.conn.execute(
            "DELETE FROM secrets WHERE kind = ?1",
            params![UNSUPPORTED_SECRET_PASSWORD],
        )?;
        self.ensure_default_templates()?;
        Ok(())
    }

    pub fn load_inventory(&self) -> Result<Inventory> {
        let templates = self.load_templates()?;
        let mut stmt = self.conn.prepare(
            "SELECT alias, display_name, group_name, hostname, user, port, identity_file, template, proxy_jump,
                    auto_reconnect
             FROM hosts ORDER BY alias",
        )?;
        let mut rows = stmt.query([])?;
        let mut hosts = Vec::new();
        while let Some(row) = rows.next()? {
            let alias: String = row.get(0)?;
            let port: Option<i64> = row.get(5)?;
            let mut host = Host::new(alias.clone(), row.get(3)?);
            host.display_name = row.get(1)?;
            host.group = row.get(2)?;
            host.user = row.get(4)?;
            host.port = port.and_then(|value| u16::try_from(value).ok());
            host.private_key_source_path = row.get(6)?;
            host.template = row.get(7)?;
            host.proxy_jump = row.get(8)?;
            host.auto_reconnect = row.get::<_, i64>(9)? != 0;
            host.tags = self.load_ordered_values("host_tags", "tag", &alias)?;
            host.local_forwards = self.load_forwards(&alias, "local")?;
            host.remote_forwards = self.load_forwards(&alias, "remote")?;
            host.dynamic_forwards = self.load_forwards(&alias, "dynamic")?;
            host.options = self.load_ordered_values("host_options", "value", &alias)?;
            host.has_private_key = self.has_secret(&alias, SECRET_PRIVATE_KEY)?;
            hosts.push(host);
        }

        let inventory = Inventory {
            version: 1,
            templates,
            hosts,
        };
        inventory.validate()?;
        Ok(inventory)
    }

    pub fn save_inventory(&mut self, inventory: &Inventory) -> Result<()> {
        inventory.validate()?;
        let tx = self.conn.transaction()?;
        tx.execute("DELETE FROM host_tags", [])?;
        tx.execute("DELETE FROM host_forwards", [])?;
        tx.execute("DELETE FROM host_options", [])?;
        tx.execute("DELETE FROM templates", [])?;

        for (name, template) in &inventory.templates {
            tx.execute(
                "INSERT INTO templates(name, argv_json) VALUES (?1, ?2)",
                params![name, serde_json::to_string(&template.argv)?],
            )?;
        }

        for host in &inventory.hosts {
            tx.execute(
                "INSERT INTO hosts(alias, display_name, group_name, hostname, user, port, identity_file, template, proxy_jump, auto_reconnect)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
                 ON CONFLICT(alias) DO UPDATE SET
                    display_name = excluded.display_name,
                    group_name = excluded.group_name,
                    hostname = excluded.hostname,
                    user = excluded.user,
                    port = excluded.port,
                    identity_file = excluded.identity_file,
                    template = excluded.template,
                    proxy_jump = excluded.proxy_jump,
                    auto_reconnect = excluded.auto_reconnect",
                params![
                    host.alias,
                    host.display_name,
                    host.group,
                    host.hostname,
                    host.user,
                    host.port.map(i64::from),
                    host.private_key_source_path,
                    host.template,
                    host.proxy_jump,
                    if host.auto_reconnect { 1_i64 } else { 0_i64 },
                ],
            )?;
            insert_ordered_values(&tx, "host_tags", "tag", &host.alias, &host.tags)?;
            insert_forwards(&tx, &host.alias, "local", &host.local_forwards)?;
            insert_forwards(&tx, &host.alias, "remote", &host.remote_forwards)?;
            insert_forwards(&tx, &host.alias, "dynamic", &host.dynamic_forwards)?;
            insert_ordered_values(&tx, "host_options", "value", &host.alias, &host.options)?;
        }
        prune_removed_hosts(&tx, inventory)?;
        tx.commit()?;
        Ok(())
    }

    pub fn put_secret(&self, alias: &str, kind: &str, blob: &SecretBlob) -> Result<()> {
        self.conn.execute(
            "INSERT INTO secrets(host_alias, kind, source_path, salt, nonce, ciphertext)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(host_alias, kind) DO UPDATE SET
                source_path = excluded.source_path,
                salt = excluded.salt,
                nonce = excluded.nonce,
                ciphertext = excluded.ciphertext",
            params![
                alias,
                kind,
                blob.source_path,
                blob.salt,
                blob.nonce,
                blob.ciphertext
            ],
        )?;
        Ok(())
    }

    pub fn get_secret(&self, alias: &str, kind: &str) -> Result<Option<SecretBlob>> {
        self.conn
            .query_row(
                "SELECT salt, nonce, ciphertext, source_path FROM secrets WHERE host_alias = ?1 AND kind = ?2",
                params![alias, kind],
                |row| {
                    Ok(SecretBlob {
                        salt: row.get(0)?,
                        nonce: row.get(1)?,
                        ciphertext: row.get(2)?,
                        source_path: row.get(3)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn remove_secret(&self, alias: &str, kind: &str) -> Result<bool> {
        let changed = self.conn.execute(
            "DELETE FROM secrets WHERE host_alias = ?1 AND kind = ?2",
            params![alias, kind],
        )?;
        Ok(changed > 0)
    }

    pub fn has_secret(&self, alias: &str, kind: &str) -> Result<bool> {
        let exists: i64 = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM secrets WHERE host_alias = ?1 AND kind = ?2)",
            params![alias, kind],
            |row| row.get(0),
        )?;
        Ok(exists != 0)
    }

    pub fn db_path_exists(path: &Path) -> bool {
        path.exists()
    }

    fn ensure_default_templates(&self) -> Result<()> {
        let defaults = Inventory::default();
        for (name, template) in defaults.templates {
            self.conn.execute(
                "INSERT OR IGNORE INTO templates(name, argv_json) VALUES (?1, ?2)",
                params![name, serde_json::to_string(&template.argv)?],
            )?;
        }
        Ok(())
    }

    fn load_templates(&self) -> Result<BTreeMap<String, Template>> {
        let mut templates = BTreeMap::new();
        let mut stmt = self
            .conn
            .prepare("SELECT name, argv_json FROM templates ORDER BY name")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in rows {
            let (name, argv_json) = row?;
            let argv = serde_json::from_str(&argv_json)?;
            templates.insert(name, Template { argv });
        }
        if templates.is_empty() {
            templates = Inventory::default().templates;
        }
        Ok(templates)
    }

    fn load_ordered_values(&self, table: &str, column: &str, alias: &str) -> Result<Vec<String>> {
        let sql = format!("SELECT {column} FROM {table} WHERE host_alias = ?1 ORDER BY position");
        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params![alias], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn load_forwards(&self, alias: &str, kind: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT value FROM host_forwards WHERE host_alias = ?1 AND kind = ?2 ORDER BY position",
        )?;
        let rows = stmt.query_map(params![alias, kind], |row| row.get::<_, String>(0))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    fn migrate_schema_v2(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
ALTER TABLE hosts ADD COLUMN use_cert_auth INTEGER NOT NULL DEFAULT 0;
ALTER TABLE hosts ADD COLUMN cert_principals_json TEXT NOT NULL DEFAULT '[]';
ALTER TABLE hosts ADD COLUMN cert_validity TEXT;

CREATE TABLE IF NOT EXISTS global_secrets (
    kind TEXT PRIMARY KEY NOT NULL,
    source_path TEXT,
    salt BLOB NOT NULL,
    nonce BLOB NOT NULL,
    ciphertext BLOB NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY NOT NULL,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS recordings (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    alias TEXT NOT NULL,
    path TEXT NOT NULL,
    started_at_unix INTEGER NOT NULL,
    duration_ms INTEGER NOT NULL,
    exit_code INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS cert_issuances (
    serial INTEGER PRIMARY KEY AUTOINCREMENT,
    alias TEXT NOT NULL,
    principal TEXT NOT NULL,
    issued_at_unix INTEGER NOT NULL,
    valid_after TEXT NOT NULL,
    valid_before TEXT NOT NULL
);

PRAGMA user_version = 2;
"#,
        )?;
        Ok(())
    }

    fn migrate_schema_v3(&self) -> Result<()> {
        self.conn.execute_batch(
            r#"
ALTER TABLE hosts ADD COLUMN auto_reconnect INTEGER NOT NULL DEFAULT 0;
DELETE FROM global_secrets WHERE kind = 'ca_private_key';
DROP TABLE IF EXISTS recordings;
DROP TABLE IF EXISTS cert_issuances;
DROP TABLE IF EXISTS global_secrets;
DROP TABLE IF EXISTS settings;
DROP TABLE IF EXISTS history;
PRAGMA user_version = 3;
"#,
        )?;
        Ok(())
    }
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

fn insert_ordered_values(
    tx: &rusqlite::Transaction<'_>,
    table: &str,
    column: &str,
    alias: &str,
    values: &[String],
) -> Result<()> {
    let sql = format!("INSERT INTO {table}(host_alias, position, {column}) VALUES (?1, ?2, ?3)");
    for (idx, value) in values.iter().enumerate() {
        tx.execute(&sql, params![alias, idx as i64, value])?;
    }
    Ok(())
}

fn prune_removed_hosts(tx: &rusqlite::Transaction<'_>, inventory: &Inventory) -> Result<()> {
    let aliases = inventory
        .hosts
        .iter()
        .map(|host| host.alias.as_str())
        .collect::<Vec<_>>();
    let existing = {
        let mut stmt = tx.prepare("SELECT alias FROM hosts")?;
        stmt.query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?
    };
    for alias in existing {
        if !aliases.iter().any(|candidate| *candidate == alias) {
            tx.execute("DELETE FROM hosts WHERE alias = ?1", params![alias])?;
        }
    }
    Ok(())
}

fn insert_forwards(
    tx: &rusqlite::Transaction<'_>,
    alias: &str,
    kind: &str,
    values: &[String],
) -> Result<()> {
    for (idx, value) in values.iter().enumerate() {
        tx.execute(
            "INSERT INTO host_forwards(host_alias, kind, position, value) VALUES (?1, ?2, ?3, ?4)",
            params![alias, kind, idx as i64, value],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::AppPaths;
    use tempfile::tempdir;

    fn paths(root: &Path) -> AppPaths {
        AppPaths {
            db: root.join("data/sshnav.db"),
            secret_key: root.join("data/secret.key"),
            runtime_dir: root.join("runtime"),
            generated_config: root.join(".ssh/sshnav.generated"),
            ssh_config: root.join(".ssh/config"),
        }
    }

    #[test]
    fn creates_schema_and_round_trips_hosts() {
        let dir = tempdir().unwrap();
        let paths = paths(dir.path());
        let mut store = Store::open(&paths).unwrap();
        let mut inventory = Inventory::default();
        let mut host = Host::new("prod".into(), "10.0.0.10".into());
        host.user = Some("ubuntu".into());
        host.tags = vec!["prod".into(), "db".into()];
        host.local_forwards = vec!["15432 localhost:5432".into()];
        inventory.hosts.push(host);

        store.save_inventory(&inventory).unwrap();
        let loaded = store.load_inventory().unwrap();

        assert_eq!(loaded.hosts.len(), 1);
        assert_eq!(loaded.hosts[0].alias, "prod");
        assert_eq!(loaded.hosts[0].tags, vec!["prod", "db"]);
        assert_eq!(loaded.hosts[0].local_forwards, vec!["15432 localhost:5432"]);
    }

    #[test]
    fn stores_private_key_presence_without_exposing_secret() {
        let dir = tempdir().unwrap();
        let paths = paths(dir.path());
        let mut store = Store::open(&paths).unwrap();
        let mut inventory = Inventory::default();
        let mut host = Host::new("prod".into(), "example.com".into());
        host.user = Some("ubuntu".into());
        inventory.hosts.push(host);
        store.save_inventory(&inventory).unwrap();
        let blob = SecretBlob {
            salt: vec![1; 16],
            nonce: vec![2; 24],
            ciphertext: vec![3; 8],
            source_path: None,
        };

        store.put_secret("prod", SECRET_PRIVATE_KEY, &blob).unwrap();
        let loaded = store.load_inventory().unwrap();

        assert!(loaded.hosts[0].has_private_key);
    }

    #[test]
    fn purges_unsupported_password_secrets_on_open() {
        let dir = tempdir().unwrap();
        let paths = paths(dir.path());
        let mut store = Store::open(&paths).unwrap();
        let mut inventory = Inventory::default();
        let mut host = Host::new("prod".into(), "example.com".into());
        host.user = Some("ubuntu".into());
        inventory.hosts.push(host);
        store.save_inventory(&inventory).unwrap();
        let blob = SecretBlob {
            salt: vec![1; 16],
            nonce: vec![2; 24],
            ciphertext: vec![3; 8],
            source_path: None,
        };
        store.put_secret("prod", "password", &blob).unwrap();
        drop(store);

        let reopened = Store::open(&paths).unwrap();

        assert!(reopened.get_secret("prod", "password").unwrap().is_none());
    }

    #[test]
    fn save_inventory_prunes_removed_hosts() {
        let dir = tempdir().unwrap();
        let paths = paths(dir.path());
        let mut store = Store::open(&paths).unwrap();
        let mut inventory = Inventory::default();
        let mut host = Host::new("prod".into(), "example.com".into());
        host.user = Some("ubuntu".into());
        inventory.hosts.push(host);
        store.save_inventory(&inventory).unwrap();

        inventory.hosts.clear();
        store.save_inventory(&inventory).unwrap();
        let loaded = store.load_inventory().unwrap();

        assert!(loaded.hosts.is_empty());
    }
}
