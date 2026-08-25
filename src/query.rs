use crate::inventory::Host;
use std::collections::HashMap;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostQuery {
    pub groups: Vec<String>,
    pub tags: Vec<String>,
    pub users: Vec<String>,
    pub unreachable_only: bool,
    pub reachable_only: bool,
    pub text: String,
}

impl HostQuery {
    pub fn uses_reachability(&self) -> bool {
        self.unreachable_only || self.reachable_only
    }
}

pub fn parse_query(input: &str) -> HostQuery {
    let mut query = HostQuery::default();
    let mut text = Vec::new();
    for token in input.split_whitespace() {
        if let Some(value) = prefixed_value(token, &["g", "group"]) {
            if !value.is_empty() {
                query.groups.push(value);
            }
        } else if let Some(value) = prefixed_value(token, &["t", "tag"]) {
            if !value.is_empty() {
                query.tags.push(value);
            }
        } else if let Some(value) = prefixed_value(token, &["u", "user"]) {
            if !value.is_empty() {
                query.users.push(value);
            }
        } else if is_flag_token(token, "unreachable") {
            query.unreachable_only = true;
        } else if is_flag_token(token, "reachable") {
            query.reachable_only = true;
        } else {
            text.push(token);
        }
    }
    query.text = text.join(" ");
    query
}

pub fn host_matches(host: &Host, query: &HostQuery, reachability: Option<&str>) -> bool {
    if !query.groups.iter().all(|group| {
        host.group
            .as_deref()
            .is_some_and(|value| contains_ci(value, group))
    }) {
        return false;
    }
    if !query.tags.iter().all(|tag| {
        host.tags
            .iter()
            .any(|candidate| contains_ci(candidate, tag))
    }) {
        return false;
    }
    if !query.users.iter().all(|user| {
        host.user
            .as_deref()
            .is_some_and(|value| contains_ci(value, user))
    }) {
        return false;
    }
    if query.unreachable_only && reachability != Some("unreachable") {
        return false;
    }
    if query.reachable_only && reachability != Some("reachable") {
        return false;
    }
    true
}

pub fn filter_hosts<'a>(
    hosts: &'a [Host],
    query: &HostQuery,
    reachability: &HashMap<String, String>,
) -> Vec<&'a Host> {
    hosts
        .iter()
        .filter(|host| {
            host_matches(
                host,
                query,
                reachability.get(&host.alias).map(String::as_str),
            )
        })
        .collect()
}

fn prefixed_value(token: &str, names: &[&str]) -> Option<String> {
    let (name, value) = token.split_once(':')?;
    names
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(name))
        .then(|| value.to_string())
}

fn is_flag_token(token: &str, name: &str) -> bool {
    if token.eq_ignore_ascii_case(name) {
        return true;
    }
    token
        .split_once(':')
        .is_some_and(|(prefix, value)| prefix.eq_ignore_ascii_case(name) && value.is_empty())
}

fn contains_ci(haystack: &str, needle: &str) -> bool {
    haystack
        .to_ascii_lowercase()
        .contains(&needle.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(alias: &str, group: Option<&str>, user: Option<&str>, tags: &[&str]) -> Host {
        let mut host = Host::new(alias.into(), format!("{alias}.example"));
        host.group = group.map(ToOwned::to_owned);
        host.user = user.map(ToOwned::to_owned);
        host.tags = tags.iter().map(|tag| (*tag).to_string()).collect();
        host
    }

    #[test]
    fn parses_structured_prefixes_and_remaining_text() {
        let query = parse_query("g:prod t:db t:oracle u:ubuntu unreachable: api");
        assert_eq!(query.groups, vec!["prod"]);
        assert_eq!(query.tags, vec!["db", "oracle"]);
        assert_eq!(query.users, vec!["ubuntu"]);
        assert!(query.unreachable_only);
        assert_eq!(query.text, "api");
    }

    #[test]
    fn matches_group_tag_and_user_substrings() {
        let prod = host("prod-db", Some("work/prod"), Some("ubuntu"), &["database"]);
        let query = parse_query("g:prod t:data u:buntu");
        assert!(host_matches(&prod, &query, None));
        assert!(!host_matches(
            &host("staging", Some("dev"), Some("root"), &["web"]),
            &query,
            None
        ));
    }

    #[test]
    fn unreachable_prefix_requires_confirmed_status() {
        let host = host("offline", Some("lab"), Some("ubuntu"), &[]);
        let query = parse_query("unreachable:");
        assert!(host_matches(&host, &query, Some("unreachable")));
        assert!(!host_matches(&host, &query, Some("checking")));
        assert!(!host_matches(&host, &query, Some("reachable")));
    }
}
