use crate::inventory::{Host, Inventory};
use anyhow::{Context, Result, bail};
use std::collections::HashSet;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HopKind {
    Proxy,
    Target,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HopStatus {
    Reachable(u128),
    Unreachable(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HopDiagnostic {
    pub kind: HopKind,
    pub label: String,
    pub user: Option<String>,
    pub hostname: String,
    pub port: u16,
    pub status: HopStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HopTarget {
    kind: HopKind,
    label: String,
    user: Option<String>,
    hostname: String,
    port: u16,
    argument: String,
}

pub fn resolve_proxy_jump(inventory: &Inventory, host: &Host) -> Result<Option<String>> {
    let mut visited = HashSet::from([host.alias.clone()]);
    let mut targets = Vec::new();
    collect_host_chain(inventory, host, &mut visited, &mut targets)?;
    Ok((!targets.is_empty()).then(|| {
        targets
            .into_iter()
            .map(|target| target.argument)
            .collect::<Vec<_>>()
            .join(",")
    }))
}

pub fn diagnose_alias(inventory: &Inventory, alias: &str) -> Result<Vec<HopDiagnostic>> {
    let host = inventory
        .find_host(alias)
        .with_context(|| format!("unknown host alias {alias:?}"))?;
    let mut visited = HashSet::from([host.alias.clone()]);
    let mut targets = Vec::new();
    collect_host_chain(inventory, host, &mut visited, &mut targets)?;
    targets.push(HopTarget {
        kind: HopKind::Target,
        label: host.alias.clone(),
        user: host.user.clone(),
        hostname: host.hostname.clone(),
        port: host.port.unwrap_or(22),
        argument: String::new(),
    });

    Ok(targets
        .into_iter()
        .map(|target| {
            let status = check_tcp(&target.hostname, target.port, Duration::from_millis(750));
            HopDiagnostic {
                kind: target.kind,
                label: target.label,
                user: target.user,
                hostname: target.hostname,
                port: target.port,
                status,
            }
        })
        .collect())
}

pub fn print_alias_diagnostics(inventory: &Inventory, alias: &str) -> Result<i32> {
    let diagnostics = diagnose_alias(inventory, alias)?;
    for (idx, hop) in diagnostics.iter().enumerate() {
        let kind = match hop.kind {
            HopKind::Proxy => "proxy",
            HopKind::Target => "target",
        };
        let user = hop.user.as_deref().unwrap_or("-");
        let status = match &hop.status {
            HopStatus::Reachable(ms) => format!("reachable in {ms} ms"),
            HopStatus::Unreachable(message) => format!("unreachable: {message}"),
        };
        println!(
            "{:<2} {:<7} {:<20} {:<12} {}:{}  {}",
            idx + 1,
            kind,
            hop.label,
            user,
            hop.hostname,
            hop.port,
            status
        );
    }
    Ok(
        if diagnostics
            .iter()
            .any(|hop| matches!(hop.status, HopStatus::Unreachable(_)))
        {
            1
        } else {
            0
        },
    )
}

fn collect_host_chain(
    inventory: &Inventory,
    host: &Host,
    visited: &mut HashSet<String>,
    out: &mut Vec<HopTarget>,
) -> Result<()> {
    let Some(proxy_jump) = host.proxy_jump.as_deref() else {
        return Ok(());
    };
    for raw_hop in proxy_jump
        .split(',')
        .map(str::trim)
        .filter(|hop| !hop.is_empty())
    {
        if let Some(proxy_host) = inventory.find_host(raw_hop) {
            if !visited.insert(proxy_host.alias.clone()) {
                bail!("proxy jump cycle detected at {:?}", proxy_host.alias);
            }
            collect_host_chain(inventory, proxy_host, visited, out)?;
            out.push(HopTarget {
                kind: HopKind::Proxy,
                label: proxy_host.alias.clone(),
                user: proxy_host.user.clone(),
                hostname: proxy_host.hostname.clone(),
                port: proxy_host.port.unwrap_or(22),
                argument: proxy_argument(proxy_host),
            });
            visited.remove(&proxy_host.alias);
        } else {
            out.push(parse_literal_proxy(raw_hop));
        }
    }
    Ok(())
}

fn proxy_argument(host: &Host) -> String {
    let hostname = if host.hostname.contains(':') && !host.hostname.starts_with('[') {
        format!("[{}]", host.hostname)
    } else {
        host.hostname.clone()
    };
    let target = match &host.user {
        Some(user) => format!("{user}@{hostname}"),
        None => hostname,
    };
    match host.port {
        Some(port) if port != 22 => format!("{target}:{port}"),
        _ => target,
    }
}

fn parse_literal_proxy(raw: &str) -> HopTarget {
    let (user, rest) = raw
        .rsplit_once('@')
        .map(|(user, rest)| (Some(user.to_string()), rest))
        .unwrap_or((None, raw));
    let (hostname, port) = if let Some(bracketed) = rest.strip_prefix('[') {
        bracketed
            .split_once(']')
            .map(|(hostname, suffix)| {
                let port = suffix
                    .strip_prefix(':')
                    .and_then(|port| port.parse().ok())
                    .unwrap_or(22);
                (hostname, port)
            })
            .unwrap_or((rest, 22))
    } else if rest.matches(':').count() == 1 {
        rest.rsplit_once(':')
            .and_then(|(hostname, port)| port.parse::<u16>().ok().map(|port| (hostname, port)))
            .unwrap_or((rest, 22))
    } else {
        (rest, 22)
    };
    HopTarget {
        kind: HopKind::Proxy,
        label: raw.to_string(),
        user,
        hostname: hostname.to_string(),
        port,
        argument: raw.to_string(),
    }
}

fn check_tcp(hostname: &str, port: u16, timeout: Duration) -> HopStatus {
    let started = Instant::now();
    let addrs = match (hostname, port).to_socket_addrs() {
        Ok(addrs) => addrs.collect::<Vec<_>>(),
        Err(err) => return HopStatus::Unreachable(format!("DNS failed: {err}")),
    };
    if addrs.is_empty() {
        return HopStatus::Unreachable("DNS returned no addresses".into());
    }
    let mut last_error = None;
    for addr in addrs {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(_) => return HopStatus::Reachable(started.elapsed().as_millis()),
            Err(err) => last_error = Some(err),
        }
    }
    HopStatus::Unreachable(
        last_error
            .map(|err| err.to_string())
            .unwrap_or_else(|| "connection failed".into()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_literal_proxy_jump() {
        let hop = parse_literal_proxy("ubuntu@bastion.example.com:2222");

        assert_eq!(hop.user.as_deref(), Some("ubuntu"));
        assert_eq!(hop.hostname, "bastion.example.com");
        assert_eq!(hop.port, 2222);
    }

    #[test]
    fn expands_db_proxy_aliases_before_target() {
        let mut inventory = Inventory::default();
        let mut bastion = Host::new("bastion".into(), "192.0.2.10".into());
        bastion.user = Some("ubuntu".into());
        let mut app = Host::new("app".into(), "192.0.2.11".into());
        app.user = Some("ubuntu".into());
        app.proxy_jump = Some("bastion".into());
        inventory.hosts = vec![app, bastion];

        let mut targets = Vec::new();
        let mut visited = HashSet::new();
        let app = inventory.find_host("app").unwrap();
        collect_host_chain(&inventory, app, &mut visited, &mut targets).unwrap();

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].label, "bastion");
    }

    #[test]
    fn resolves_nested_saved_and_literal_proxy_jumps() {
        let mut edge = Host::new("edge".into(), "2001:db8::10".into());
        edge.user = Some("jump".into());
        edge.port = Some(2222);
        edge.proxy_jump = Some("ops@first.example:2200".into());
        let mut app = Host::new("app".into(), "app.example".into());
        app.proxy_jump = Some("edge".into());
        let inventory = Inventory {
            hosts: vec![app, edge],
            ..Inventory::default()
        };

        assert_eq!(
            resolve_proxy_jump(&inventory, inventory.find_host("app").unwrap()).unwrap(),
            Some("ops@first.example:2200,jump@[2001:db8::10]:2222".into())
        );
    }

    #[test]
    fn rejects_proxy_jump_cycles() {
        let mut first = Host::new("first".into(), "first.example".into());
        first.proxy_jump = Some("second".into());
        let mut second = Host::new("second".into(), "second.example".into());
        second.proxy_jump = Some("first".into());
        let inventory = Inventory {
            hosts: vec![first, second],
            ..Inventory::default()
        };

        let error = resolve_proxy_jump(&inventory, inventory.find_host("first").unwrap())
            .unwrap_err()
            .to_string();
        assert!(error.contains("cycle"));
    }
}
