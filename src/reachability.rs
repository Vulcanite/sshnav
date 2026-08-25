use crate::inventory::Host;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const CACHE_TTL_SECS: u64 = 90;
const PROBE_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReachabilityCache {
    #[serde(default)]
    pub entries: HashMap<String, CacheEntry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheEntry {
    pub hostname: String,
    pub port: u16,
    pub status: String,
    pub checked_at: u64,
}

impl ReachabilityCache {
    pub fn load(path: &Path) -> Self {
        fs::read_to_string(path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        let payload = serde_json::to_string_pretty(self)?;
        fs::write(path, payload).with_context(|| format!("could not write {}", path.display()))?;
        set_owner_only_permissions(path)?;
        Ok(())
    }

    pub fn fresh_status(&self, host: &Host, now: u64) -> Option<&str> {
        let entry = self.entries.get(&host.alias)?;
        if !entry.matches(host) || !entry.is_fresh(now) {
            return None;
        }
        Some(entry.status.as_str())
    }

    pub fn upsert(&mut self, host: &Host, status: &str, now: u64) {
        if status == "checking" {
            return;
        }
        self.entries.insert(
            host.alias.clone(),
            CacheEntry {
                hostname: host.hostname.clone(),
                port: host.port.unwrap_or(22),
                status: status.to_string(),
                checked_at: now,
            },
        );
    }

    pub fn offline_groups(&self, hosts: &[Host], now: u64) -> HashSet<String> {
        let mut groups: HashMap<String, (bool, bool)> = HashMap::new();
        for host in hosts {
            let Some(group) = host.group.as_ref() else {
                continue;
            };
            let Some(status) = self.fresh_status(host, now) else {
                continue;
            };
            let slot = groups.entry(group.clone()).or_insert((false, false));
            match status {
                "reachable" => slot.0 = true,
                "unreachable" => slot.1 = true,
                _ => {}
            }
        }
        groups
            .into_iter()
            .filter_map(|(group, (reachable, unreachable))| {
                (unreachable && !reachable).then_some(group)
            })
            .collect()
    }
}

impl CacheEntry {
    fn matches(&self, host: &Host) -> bool {
        self.hostname == host.hostname && self.port == host.port.unwrap_or(22)
    }

    fn is_fresh(&self, now: u64) -> bool {
        now.saturating_sub(self.checked_at) <= CACHE_TTL_SECS
    }
}

pub fn cache_path(db_path: &Path) -> PathBuf {
    db_path
        .parent()
        .unwrap_or(db_path)
        .join("reachability.json")
}

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

pub fn probe(hostname: &str, port: u16) -> String {
    let Ok(mut addrs) = (hostname, port).to_socket_addrs() else {
        return "unknown".to_string();
    };
    let Some(addr) = addrs.next() else {
        return "unknown".to_string();
    };
    match TcpStream::connect_timeout(&addr, PROBE_TIMEOUT) {
        Ok(_) => "reachable".to_string(),
        Err(_) => "unreachable".to_string(),
    }
}

#[cfg(unix)]
fn set_owner_only_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .with_context(|| format!("could not set owner-only permissions on {}", path.display()))
}

#[cfg(not(unix))]
fn set_owner_only_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(alias: &str, group: Option<&str>) -> Host {
        let mut host = Host::new(alias.into(), format!("{alias}.example"));
        host.group = group.map(ToOwned::to_owned);
        host.user = Some("ubuntu".into());
        host
    }

    #[test]
    fn skips_probes_for_groups_that_recently_looked_offline() {
        let mut cache = ReachabilityCache::default();
        let now = 1_700_000_000;
        let db = host("db", Some("prod"));
        let api = host("api", Some("prod"));
        let lab = host("lab", Some("dev"));
        cache.upsert(&db, "unreachable", now);
        cache.upsert(&lab, "reachable", now);

        let offline = cache.offline_groups(&[db, api, lab], now);
        assert!(offline.contains("prod"));
        assert!(!offline.contains("dev"));
        assert_eq!(
            cache.fresh_status(&host("db", Some("prod")), now),
            Some("unreachable")
        );
        assert_eq!(
            cache.fresh_status(&host("db", Some("prod")), now + CACHE_TTL_SECS + 1),
            None
        );
    }

    #[test]
    fn cache_misses_when_hostname_or_port_changes() {
        let mut cache = ReachabilityCache::default();
        let now = 10;
        let mut db = host("db", Some("prod"));
        cache.upsert(&db, "reachable", now);
        db.hostname = "db-2.example".into();
        assert_eq!(cache.fresh_status(&db, now), None);
    }
}
