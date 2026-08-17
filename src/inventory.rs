use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::net::IpAddr;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InventoryError {
    #[error("host alias cannot be empty")]
    EmptyAlias,
    #[error("host alias {0:?} cannot begin with '-'")]
    FlagLikeAlias(String),
    #[error("duplicate host alias {0:?}")]
    DuplicateAlias(String),
    #[error("{field} for host {alias:?} contains a control character")]
    ControlCharacter { alias: String, field: &'static str },
    #[error("hostname {hostname:?} for host {alias:?} is not a valid IP address or DNS hostname")]
    InvalidHostname { alias: String, hostname: String },
    #[error("user is required for host {0:?}")]
    MissingUser(String),
    #[error("template {0:?} must contain at least one argv element")]
    EmptyTemplate(String),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Inventory {
    #[serde(default = "default_version")]
    pub version: u32,
    #[serde(default)]
    pub templates: BTreeMap<String, Template>,
    #[serde(default)]
    pub hosts: Vec<Host>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Template {
    pub argv: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Host {
    pub alias: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub hostname: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub private_key_source_path: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub has_private_key: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub proxy_jump: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub local_forwards: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remote_forwards: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dynamic_forwards: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub auto_reconnect: bool,
}

impl Default for Inventory {
    fn default() -> Self {
        let mut templates = BTreeMap::new();
        templates.insert(
            "ssh".to_string(),
            Template {
                argv: vec![
                    "ssh".to_string(),
                    "-F".to_string(),
                    "{generated_config}".to_string(),
                    "{alias}".to_string(),
                ],
            },
        );
        templates.insert(
            "mosh".to_string(),
            Template {
                argv: vec!["mosh".to_string(), "{user_at_hostname}".to_string()],
            },
        );

        Self {
            version: default_version(),
            templates,
            hosts: Vec::new(),
        }
    }
}

impl Host {
    pub fn new(alias: String, hostname: String) -> Self {
        Self {
            alias,
            display_name: None,
            group: None,
            tags: Vec::new(),
            hostname,
            user: None,
            port: None,
            private_key_source_path: None,
            has_private_key: false,
            template: None,
            proxy_jump: None,
            local_forwards: Vec::new(),
            remote_forwards: Vec::new(),
            dynamic_forwards: Vec::new(),
            options: Vec::new(),
            auto_reconnect: false,
        }
    }
}

impl Inventory {
    pub fn validate(&self) -> Result<(), InventoryError> {
        let mut aliases = HashSet::new();
        for (name, template) in &self.templates {
            if template.argv.is_empty() {
                return Err(InventoryError::EmptyTemplate(name.clone()));
            }
        }
        for host in &self.hosts {
            validate_alias(&host.alias)?;
            if !aliases.insert(host.alias.clone()) {
                return Err(InventoryError::DuplicateAlias(host.alias.clone()));
            }
            validate_generated_field(&host.alias, "alias", &host.alias)?;
            validate_generated_field(&host.alias, "hostname", &host.hostname)?;
            validate_hostname(&host.alias, &host.hostname)?;
            validate_optional(&host.alias, "display_name", &host.display_name)?;
            validate_optional(&host.alias, "group", &host.group)?;
            validate_required_user(&host.alias, &host.user)?;
            validate_optional(
                &host.alias,
                "private_key_source_path",
                &host.private_key_source_path,
            )?;
            validate_optional(&host.alias, "template", &host.template)?;
            validate_optional(&host.alias, "proxy_jump", &host.proxy_jump)?;
            validate_many(&host.alias, "tags", &host.tags)?;
            validate_many(&host.alias, "local_forwards", &host.local_forwards)?;
            validate_many(&host.alias, "remote_forwards", &host.remote_forwards)?;
            validate_many(&host.alias, "dynamic_forwards", &host.dynamic_forwards)?;
            validate_many(&host.alias, "options", &host.options)?;
        }
        Ok(())
    }

    pub fn find_host(&self, alias: &str) -> Option<&Host> {
        self.hosts.iter().find(|host| host.alias == alias)
    }

    pub fn find_host_mut(&mut self, alias: &str) -> Option<&mut Host> {
        self.hosts.iter_mut().find(|host| host.alias == alias)
    }

    pub fn upsert_imported_hosts(&mut self, imported: Vec<Host>) {
        for imported_host in imported {
            if let Some(existing) = self.find_host_mut(&imported_host.alias) {
                *existing = imported_host;
            } else {
                self.hosts.push(imported_host);
            }
        }
        self.hosts.sort_by(|a, b| a.alias.cmp(&b.alias));
    }
}

fn validate_alias(alias: &str) -> Result<(), InventoryError> {
    if alias.is_empty() {
        return Err(InventoryError::EmptyAlias);
    }
    if alias.starts_with('-')
        || alias == "."
        || alias == ".."
        || alias.contains("..")
        || alias.contains(['/', '\\', '*', '?', '[', ']', '!', ' '])
    {
        return Err(InventoryError::FlagLikeAlias(alias.to_string()));
    }
    Ok(())
}

fn validate_optional(
    alias: &str,
    field: &'static str,
    value: &Option<String>,
) -> Result<(), InventoryError> {
    if let Some(value) = value {
        validate_generated_field(alias, field, value)?;
    }
    Ok(())
}

fn validate_required_user(alias: &str, value: &Option<String>) -> Result<(), InventoryError> {
    let Some(value) = value else {
        return Err(InventoryError::MissingUser(alias.to_string()));
    };
    if value.trim().is_empty() {
        return Err(InventoryError::MissingUser(alias.to_string()));
    }
    if value.starts_with('-') || value.contains(char::is_whitespace) || value.contains('@') {
        return Err(InventoryError::ControlCharacter {
            alias: alias.to_string(),
            field: "user",
        });
    }
    validate_generated_field(alias, "user", value)?;
    Ok(())
}

fn validate_many(
    alias: &str,
    field: &'static str,
    values: &[String],
) -> Result<(), InventoryError> {
    for value in values {
        validate_generated_field(alias, field, value)?;
    }
    Ok(())
}

fn validate_generated_field(
    alias: &str,
    field: &'static str,
    value: &str,
) -> Result<(), InventoryError> {
    if value.chars().any(char::is_control) {
        return Err(InventoryError::ControlCharacter {
            alias: alias.to_string(),
            field,
        });
    }
    Ok(())
}

fn validate_hostname(alias: &str, hostname: &str) -> Result<(), InventoryError> {
    if hostname.parse::<IpAddr>().is_ok() || is_valid_dns_hostname(hostname) {
        return Ok(());
    }
    Err(InventoryError::InvalidHostname {
        alias: alias.to_string(),
        hostname: hostname.to_string(),
    })
}

fn is_valid_dns_hostname(hostname: &str) -> bool {
    if hostname.is_empty() || hostname.len() > 253 {
        return false;
    }
    let trimmed = hostname.strip_suffix('.').unwrap_or(hostname);
    trimmed.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && !label.starts_with('-')
            && !label.ends_with('-')
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
    })
}

fn default_version() -> u32 {
    1
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_duplicate_aliases() {
        let mut inventory = Inventory::default();
        let mut first = Host::new("prod".into(), "one".into());
        first.user = Some("ubuntu".into());
        let mut second = Host::new("prod".into(), "two".into());
        second.user = Some("ubuntu".into());
        inventory.hosts.push(first);
        inventory.hosts.push(second);

        assert_eq!(
            inventory.validate().unwrap_err(),
            InventoryError::DuplicateAlias("prod".into())
        );
    }

    #[test]
    fn rejects_flag_like_aliases() {
        let mut inventory = Inventory::default();
        inventory
            .hosts
            .push(Host::new("-oProxyCommand=bad".into(), "host".into()));

        assert_eq!(
            inventory.validate().unwrap_err(),
            InventoryError::FlagLikeAlias("-oProxyCommand=bad".into())
        );
    }

    #[test]
    fn rejects_control_characters() {
        let mut inventory = Inventory::default();
        inventory
            .hosts
            .push(Host::new("prod".into(), "bad\nhost".into()));

        assert!(matches!(
            inventory.validate(),
            Err(InventoryError::ControlCharacter {
                field: "hostname",
                ..
            })
        ));
    }

    #[test]
    fn rejects_invalid_hostnames() {
        let mut inventory = Inventory::default();
        inventory
            .hosts
            .push(Host::new("prod".into(), "bad host".into()));

        assert!(matches!(
            inventory.validate(),
            Err(InventoryError::InvalidHostname { .. })
        ));
    }

    #[test]
    fn accepts_ip_addresses_and_dns_names() {
        let mut inventory = Inventory::default();
        let mut prod = Host::new("prod".into(), "10.0.0.10".into());
        prod.user = Some("ubuntu".into());
        let mut dev = Host::new("dev".into(), "dev.example.com".into());
        dev.user = Some("amish".into());
        inventory.hosts.push(prod);
        inventory.hosts.push(dev);

        assert!(inventory.validate().is_ok());
    }

    #[test]
    fn rejects_missing_user() {
        let mut inventory = Inventory::default();
        inventory
            .hosts
            .push(Host::new("prod".into(), "10.0.0.10".into()));

        assert_eq!(
            inventory.validate().unwrap_err(),
            InventoryError::MissingUser("prod".into())
        );
    }
}
