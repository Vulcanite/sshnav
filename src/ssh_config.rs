use crate::inventory::Host;
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::path::Path;

#[allow(dead_code)]
pub fn import_file_with_warnings(path: &Path) -> Result<ImportResult> {
    let contents = fs::read_to_string(path)
        .with_context(|| format!("could not read ssh config at {}", path.display()))?;
    Ok(parse_with_warnings(&contents))
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct ImportResult {
    pub hosts: Vec<Host>,
    pub placeholder_user_count: usize,
}

pub fn parse_with_warnings(contents: &str) -> ImportResult {
    parse_with_username(contents, current_os_username)
}

fn parse_with_username(
    contents: &str,
    mut username: impl FnMut() -> UsernameGuess,
) -> ImportResult {
    let mut blocks = Vec::new();
    let mut current: Option<Block> = None;

    for raw_line in contents.lines() {
        let line = strip_inline_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = split_directive(line) else {
            continue;
        };

        if key.eq_ignore_ascii_case("Host") {
            if let Some(block) = current.take() {
                blocks.push(block);
            }
            current = Some(Block::new(value));
            continue;
        }

        let Some(block) = current.as_mut() else {
            continue;
        };
        block.apply(key, value);
    }

    if let Some(block) = current {
        blocks.push(block);
    }

    let mut imported = Vec::new();
    let mut imported_aliases = HashSet::new();
    let mut placeholder_user_count = 0;
    for block in &blocks {
        for alias in block.concrete_aliases() {
            if !imported_aliases.insert(alias.clone()) {
                continue;
            }
            let effective = effective_block_for_alias(&blocks, &alias);
            let username = match effective.user {
                Some(user) => user,
                None => {
                    let guess = username();
                    placeholder_user_count += usize::from(guess.used_placeholder);
                    guess.username
                }
            };
            let mut host = Host::new(
                alias.clone(),
                effective.hostname.unwrap_or_else(|| alias.clone()),
            );
            host.user = Some(username);
            host.port = effective.port;
            host.private_key_source_path = effective.identity_file;
            host.proxy_jump = effective.proxy_jump;
            host.local_forwards = effective.local_forwards;
            host.remote_forwards = effective.remote_forwards;
            host.dynamic_forwards = effective.dynamic_forwards;
            host.options = effective.options;
            imported.push(host);
        }
    }
    ImportResult {
        hosts: imported,
        placeholder_user_count,
    }
}

fn effective_block_for_alias(blocks: &[Block], alias: &str) -> BlockValues {
    let mut effective = BlockValues::default();
    for block in blocks.iter().filter(|block| block.matches(alias)) {
        effective.apply_first_values(block);
    }
    effective
}

struct UsernameGuess {
    username: String,
    used_placeholder: bool,
}

fn current_os_username() -> UsernameGuess {
    current_os_username_from(|name| env::var(name).ok())
}

fn current_os_username_from(mut get_var: impl FnMut(&str) -> Option<String>) -> UsernameGuess {
    for name in ["USER", "LOGNAME", "USERNAME"] {
        if let Some(value) = get_var(name).filter(|value| !value.trim().is_empty()) {
            return UsernameGuess {
                username: value,
                used_placeholder: false,
            };
        }
    }
    UsernameGuess {
        username: "user".to_string(),
        used_placeholder: true,
    }
}

#[derive(Clone, Debug)]
struct Block {
    patterns: Vec<String>,
    hostname: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<String>,
    proxy_jump: Option<String>,
    local_forwards: Vec<String>,
    remote_forwards: Vec<String>,
    dynamic_forwards: Vec<String>,
    options: Vec<String>,
}

impl Block {
    fn new(patterns: &str) -> Self {
        Self {
            patterns: split_words(patterns),
            hostname: None,
            user: None,
            port: None,
            identity_file: None,
            proxy_jump: None,
            local_forwards: Vec::new(),
            remote_forwards: Vec::new(),
            dynamic_forwards: Vec::new(),
            options: Vec::new(),
        }
    }

    fn apply(&mut self, key: &str, value: &str) {
        match key.to_ascii_lowercase().as_str() {
            "hostname" => self.hostname = Some(value.to_string()),
            "user" => self.user = Some(value.to_string()),
            "port" => self.port = value.parse::<u16>().ok(),
            "identityfile" => self.identity_file = Some(value.to_string()),
            "proxyjump" => self.proxy_jump = Some(value.to_string()),
            "localforward" => self.local_forwards.push(value.to_string()),
            "remoteforward" => self.remote_forwards.push(value.to_string()),
            "dynamicforward" => self.dynamic_forwards.push(value.to_string()),
            _ => self.options.push(format!("{key} {value}")),
        }
    }

    fn concrete_aliases(&self) -> impl Iterator<Item = String> + '_ {
        self.patterns
            .iter()
            .filter(|pattern| is_concrete_host_pattern(pattern))
            .cloned()
    }

    fn matches(&self, alias: &str) -> bool {
        let mut matched = false;
        for pattern in &self.patterns {
            if let Some(pattern) = pattern.strip_prefix('!') {
                if pattern_matches(pattern, alias) {
                    return false;
                }
            } else if pattern_matches(pattern, alias) {
                matched = true;
            }
        }
        matched
    }
}

#[derive(Default)]
struct BlockValues {
    hostname: Option<String>,
    user: Option<String>,
    port: Option<u16>,
    identity_file: Option<String>,
    proxy_jump: Option<String>,
    local_forwards: Vec<String>,
    remote_forwards: Vec<String>,
    dynamic_forwards: Vec<String>,
    options: Vec<String>,
    option_keys: HashSet<String>,
}

impl BlockValues {
    fn apply_first_values(&mut self, block: &Block) {
        set_if_missing(&mut self.hostname, &block.hostname);
        set_if_missing(&mut self.user, &block.user);
        set_if_missing(&mut self.port, &block.port);
        set_if_missing(&mut self.identity_file, &block.identity_file);
        set_if_missing(&mut self.proxy_jump, &block.proxy_jump);

        self.local_forwards.extend(block.local_forwards.clone());
        self.remote_forwards.extend(block.remote_forwards.clone());
        self.dynamic_forwards.extend(block.dynamic_forwards.clone());
        for option in &block.options {
            let key = option_key(option);
            if self.option_keys.insert(key) {
                self.options.push(option.clone());
            }
        }
    }
}

fn set_if_missing<T: Clone>(target: &mut Option<T>, source: &Option<T>) {
    if target.is_none() {
        *target = source.clone();
    }
}

fn option_key(option: &str) -> String {
    option
        .split_once(char::is_whitespace)
        .map(|(key, _)| key)
        .unwrap_or(option)
        .to_ascii_lowercase()
}

fn split_directive(line: &str) -> Option<(&str, &str)> {
    let mut parts = line.splitn(2, char::is_whitespace);
    let key = parts.next()?.trim();
    let value = parts.next()?.trim();
    if key.is_empty() || value.is_empty() {
        return None;
    }
    Some((key, value))
}

fn split_words(value: &str) -> Vec<String> {
    value
        .split_whitespace()
        .map(str::trim)
        .filter(|word| !word.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn is_concrete_host_pattern(pattern: &str) -> bool {
    !pattern.starts_with('!') && !pattern.contains('*') && !pattern.contains('?')
}

fn pattern_matches(pattern: &str, alias: &str) -> bool {
    wildcard_match(
        pattern.to_ascii_lowercase().as_bytes(),
        alias.to_ascii_lowercase().as_bytes(),
    )
}

fn wildcard_match(pattern: &[u8], value: &[u8]) -> bool {
    match (pattern.split_first(), value.split_first()) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some((&b'*', rest)), _) => {
            wildcard_match(rest, value)
                || value
                    .split_first()
                    .is_some_and(|(_, value_rest)| wildcard_match(pattern, value_rest))
        }
        (Some((&b'?', rest)), Some((_, value_rest))) => wildcard_match(rest, value_rest),
        (Some((&pattern_ch, rest)), Some((&value_ch, value_rest))) if pattern_ch == value_ch => {
            wildcard_match(rest, value_rest)
        }
        _ => false,
    }
}

fn strip_inline_comment(line: &str) -> &str {
    let mut escaped = false;
    for (idx, ch) in line.char_indices() {
        if ch == '\\' {
            escaped = !escaped;
            continue;
        }
        if ch == '#' && !escaped {
            return &line[..idx];
        }
        escaped = false;
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imports_common_host_block_fields() {
        let imported = parse_with_warnings(
            r#"
Host prod-db
  HostName 10.0.0.10
  User ubuntu
  Port 2222
  IdentityFile ~/.ssh/prod
  ProxyJump bastion
  LocalForward 15432 localhost:5432
  ServerAliveInterval 60
"#,
        )
        .hosts;

        assert_eq!(imported.len(), 1);
        let host = &imported[0];
        assert_eq!(host.alias, "prod-db");
        assert_eq!(host.hostname, "10.0.0.10");
        assert_eq!(host.user.as_deref(), Some("ubuntu"));
        assert_eq!(host.port, Some(2222));
        assert_eq!(host.private_key_source_path.as_deref(), Some("~/.ssh/prod"));
        assert_eq!(host.proxy_jump.as_deref(), Some("bastion"));
        assert_eq!(host.local_forwards, vec!["15432 localhost:5432"]);
        assert_eq!(host.options, vec!["ServerAliveInterval 60"]);
    }

    #[test]
    fn skips_wildcard_only_hosts() {
        let imported = parse_with_warnings(
            r#"
Host *
  User nobody

Host *.internal ?host !blocked
  User nobody
"#,
        )
        .hosts;

        assert!(imported.is_empty());
    }

    #[test]
    fn imports_concrete_names_from_mixed_host_block() {
        let imported = parse_with_warnings(
            r#"
Host prod *.internal
  HostName prod.example.com
  User ubuntu
"#,
        )
        .hosts;

        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].alias, "prod");
    }

    #[test]
    fn inherits_wildcard_options_for_concrete_hosts() {
        let imported = parse_with_warnings(
            r#"
Host prod
  HostName prod.example.com
  User ubuntu

Host *
  ServerAliveInterval 60
  ServerAliveCountMax 3
"#,
        )
        .hosts;

        assert_eq!(imported.len(), 1);
        assert_eq!(
            imported[0].options,
            vec!["ServerAliveInterval 60", "ServerAliveCountMax 3"]
        );
    }

    #[test]
    fn first_matching_scalar_and_option_values_win() {
        let imported = parse_with_warnings(
            r#"
Host prod
  HostName prod.example.com
  User ubuntu
  ServerAliveInterval 30

Host *
  User default-user
  ServerAliveInterval 60
  IdentityFile ~/.ssh/default
"#,
        )
        .hosts;

        assert_eq!(imported.len(), 1);
        let host = &imported[0];
        assert_eq!(host.user.as_deref(), Some("ubuntu"));
        assert_eq!(
            host.private_key_source_path.as_deref(),
            Some("~/.ssh/default")
        );
        assert_eq!(host.options, vec!["ServerAliveInterval 30"]);
    }

    #[test]
    fn wildcard_blocks_can_be_excluded_with_negated_patterns() {
        let imported = parse_with_warnings(
            r#"
Host prod
  HostName prod.example.com
  User ubuntu

Host * !prod
  ServerAliveInterval 60
"#,
        )
        .hosts;

        assert_eq!(imported.len(), 1);
        assert!(imported[0].options.is_empty());
    }

    #[test]
    fn imports_hosts_without_user_using_current_os_user() {
        let imported = parse_with_warnings(
            r#"
Host prod
  HostName prod.example.com
"#,
        )
        .hosts;

        assert_eq!(imported.len(), 1);
        assert_eq!(imported[0].alias, "prod");
        let expected = current_os_username();
        assert_eq!(
            imported[0].user.as_deref(),
            Some(expected.username.as_str())
        );
    }

    #[test]
    fn username_fallback_reports_placeholder_when_os_user_is_missing() {
        let guess = current_os_username_from(|_| None);

        assert_eq!(guess.username, "user");
        assert!(guess.used_placeholder);
    }

    #[test]
    fn parse_result_counts_placeholder_user_fallbacks() {
        let result = parse_with_username(
            r#"
Host prod
  HostName prod.example.com
"#,
            || current_os_username_from(|_| None),
        );

        assert_eq!(result.hosts.len(), 1);
        assert_eq!(result.hosts[0].user.as_deref(), Some("user"));
        assert_eq!(result.placeholder_user_count, 1);
    }
}
