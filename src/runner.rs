use crate::diagnostics;
use crate::inventory::{Host, Inventory};
use crate::paths::AppPaths;
use crate::secrets;
use crate::storage::{SECRET_PRIVATE_KEY, Store};
use crate::term;
use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};
use std::thread;
use std::time::{Duration, Instant};
use tempfile::NamedTempFile;

pub fn connect_alias(
    inventory: &Inventory,
    store: &Store,
    paths: &AppPaths,
    alias: &str,
) -> Result<i32> {
    let host = inventory
        .find_host(alias)
        .with_context(|| format!("unknown host alias {alias:?}"))?;
    term::info(format!(
        "connecting to {alias} ({})",
        user_at_hostname(host)
    ));
    let proxy_jump = diagnostics::resolve_proxy_jump(inventory, host)?;
    if host.template.as_deref() == Some("mosh") {
        if proxy_jump.is_some() {
            bail!("mosh connections do not support proxy jumps; use the SSH template");
        }
        let prepared = prepare_command(store, paths, host, None)?;
        let status = run_prepared(&prepared)?;
        return Ok(status.code().unwrap_or(1));
    }

    let mut retries = 0_u8;
    loop {
        let prepared = prepare_command(store, paths, host, proxy_jump.as_deref())?;
        let started = Instant::now();
        let status = run_prepared(&prepared)?;
        let code = status.code().unwrap_or(1);
        let active_session = started.elapsed() >= Duration::from_secs(30);
        if status.success() {
            return Ok(code);
        }
        if !should_reconnect(code, host.auto_reconnect, active_session, retries) {
            return Ok(code);
        }
        let delay = 1_u64 << retries;
        retries += 1;
        term::warn(format!(
            "connection to {alias} was lost; reconnecting in {delay}s ({retries}/3, Ctrl-C to stop)"
        ));
        thread::sleep(Duration::from_secs(delay));
    }
}

pub fn send_alias(
    inventory: &Inventory,
    store: &Store,
    paths: &AppPaths,
    alias: &str,
    source: &Path,
    destination: &str,
    options: TransferOptions,
) -> Result<i32> {
    validate_send_source(source, options.recursive)?;
    transfer_alias(
        inventory,
        store,
        paths,
        alias,
        source,
        destination,
        TransferDirection::Send,
        options,
    )
}

pub fn receive_alias(
    inventory: &Inventory,
    store: &Store,
    paths: &AppPaths,
    alias: &str,
    source: &str,
    destination: &Path,
    options: TransferOptions,
) -> Result<i32> {
    transfer_alias(
        inventory,
        store,
        paths,
        alias,
        destination,
        source,
        TransferDirection::Receive,
        options,
    )
}

#[allow(clippy::too_many_arguments)]
fn transfer_alias(
    inventory: &Inventory,
    store: &Store,
    paths: &AppPaths,
    alias: &str,
    local_path: &Path,
    remote_path: &str,
    direction: TransferDirection,
    options: TransferOptions,
) -> Result<i32> {
    let host = inventory
        .find_host(alias)
        .with_context(|| format!("unknown host alias {alias:?}"))?;
    let proxy_jump = diagnostics::resolve_proxy_jump(inventory, host)?;
    let prepared = prepare_transfer_command(
        store,
        paths,
        host,
        local_path,
        remote_path,
        direction,
        options,
        proxy_jump.as_deref(),
    )?;
    Ok(run_prepared(&prepared)?.code().unwrap_or(1))
}

fn should_reconnect(exit_code: i32, enabled: bool, active_session: bool, retries: u8) -> bool {
    exit_code == 255 && enabled && active_session && retries < 3
}

pub fn render_ssh_argv(
    host: &Host,
    private_key_path: Option<PathBuf>,
    proxy_jump: Option<&str>,
) -> Vec<String> {
    let mut argv = vec!["ssh".to_string()];
    if let Some(port) = host.port {
        argv.push("-p".to_string());
        argv.push(port.to_string());
    }
    if let Some(path) =
        private_key_path.or_else(|| host.private_key_source_path.as_ref().map(PathBuf::from))
    {
        argv.push("-i".to_string());
        argv.push(path.display().to_string());
    }
    if let Some(proxy_jump) = proxy_jump {
        argv.push("-J".to_string());
        argv.push(proxy_jump.to_string());
    }
    for forward in &host.local_forwards {
        argv.push("-L".to_string());
        argv.push(forward.clone());
    }
    for forward in &host.remote_forwards {
        argv.push("-R".to_string());
        argv.push(forward.clone());
    }
    for forward in &host.dynamic_forwards {
        argv.push("-D".to_string());
        argv.push(forward.clone());
    }
    for option in managed_ssh_options() {
        argv.push("-o".to_string());
        argv.push(option.to_string());
    }
    for option in &host.options {
        if is_safe_option(option) {
            argv.push("-o".to_string());
            argv.push(option.clone());
        }
    }
    argv.push(user_at_hostname(host));
    argv
}

fn render_scp_argv(
    host: &Host,
    private_key_path: Option<PathBuf>,
    local_path: &Path,
    remote_path: &str,
    direction: TransferDirection,
    recursive: bool,
    proxy_jump: Option<&str>,
) -> Result<Vec<String>> {
    validate_remote_path(remote_path)?;

    let mut argv = vec!["scp".to_string()];
    append_transfer_ssh_args(&mut argv, host, private_key_path, proxy_jump, "-P");
    if recursive {
        argv.push("-r".to_string());
    }

    let local = local_path.display().to_string();
    let remote = remote_target(host, remote_path);
    match direction {
        TransferDirection::Send => argv.extend([local, remote]),
        TransferDirection::Receive => argv.extend([remote, local]),
    }
    Ok(argv)
}

fn render_rsync_argv(
    host: &Host,
    private_key_path: Option<PathBuf>,
    local_path: &Path,
    remote_path: &str,
    direction: TransferDirection,
    recursive: bool,
    proxy_jump: Option<&str>,
) -> Result<Vec<String>> {
    validate_remote_path(remote_path)?;

    let mut ssh = vec!["ssh".to_string()];
    append_transfer_ssh_args(&mut ssh, host, private_key_path, proxy_jump, "-p");
    let rsh = render_rsync_rsh(&ssh)?;

    let mut argv = vec!["rsync".to_string(), "-avzP".to_string()];
    if !recursive {
        argv.push("--no-recursive".to_string());
    }
    argv.extend([
        "--protect-args".to_string(),
        "-e".to_string(),
        rsh,
        "--".to_string(),
    ]);

    let local = local_path.display().to_string();
    let remote = remote_target(host, remote_path);
    match direction {
        TransferDirection::Send => argv.extend([local, remote]),
        TransferDirection::Receive => argv.extend([remote, local]),
    }
    Ok(argv)
}

fn append_transfer_ssh_args(
    argv: &mut Vec<String>,
    host: &Host,
    private_key_path: Option<PathBuf>,
    proxy_jump: Option<&str>,
    port_flag: &str,
) {
    if let Some(port) = host.port {
        argv.extend([port_flag.to_string(), port.to_string()]);
    }
    if let Some(path) =
        private_key_path.or_else(|| host.private_key_source_path.as_ref().map(PathBuf::from))
    {
        argv.extend(["-i".to_string(), path.display().to_string()]);
    }
    if let Some(proxy_jump) = proxy_jump {
        argv.extend(["-J".to_string(), proxy_jump.to_string()]);
    }
    for option in managed_ssh_options() {
        argv.extend(["-o".to_string(), option.to_string()]);
    }
    for option in &host.options {
        if is_safe_option(option) {
            argv.extend(["-o".to_string(), option.clone()]);
        }
    }
}

fn render_rsync_rsh(argv: &[String]) -> Result<String> {
    argv.iter()
        .map(|arg| {
            if arg.chars().any(char::is_control) {
                bail!("SSH argument contains a control character");
            }
            Ok(format!("'{}'", arg.replace('\'', "''")))
        })
        .collect::<Result<Vec<_>>>()
        .map(|args| args.join(" "))
}

fn validate_remote_path(path: &str) -> Result<()> {
    if path.chars().any(char::is_control) {
        bail!("remote path contains a control character");
    }
    Ok(())
}

fn remote_target(host: &Host, path: &str) -> String {
    let hostname = if host.hostname.contains(':') {
        format!("[{}]", host.hostname)
    } else {
        host.hostname.clone()
    };
    match &host.user {
        Some(user) => format!("{user}@{hostname}:{path}"),
        None => format!("{hostname}:{path}"),
    }
}

fn validate_send_source(path: &Path, recursive: bool) -> Result<()> {
    let metadata = fs::metadata(path).with_context(|| {
        format!(
            "local source {} does not exist or cannot be accessed",
            path.display()
        )
    })?;
    if metadata.is_dir() && !recursive {
        bail!(
            "local source {} is a directory; use --recursive",
            path.display()
        );
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum TransferDirection {
    Send,
    Receive,
}

#[derive(Clone, Copy)]
pub struct TransferOptions {
    pub recursive: bool,
    pub rsync: bool,
}

fn managed_ssh_options() -> [&'static str; 9] {
    [
        "StrictHostKeyChecking=ask",
        "UserKnownHostsFile=~/.ssh/known_hosts",
        "HashKnownHosts=yes",
        "UpdateHostKeys=yes",
        "ServerAliveInterval=15",
        "ServerAliveCountMax=3",
        "TCPKeepAlive=yes",
        "ConnectTimeout=10",
        "ConnectionAttempts=3",
    ]
}

fn is_safe_option(option: &str) -> bool {
    let Some((key, _)) = option.split_once(char::is_whitespace) else {
        return false;
    };
    matches!(
        key.to_ascii_lowercase().as_str(),
        "addressfamily"
            | "batchmode"
            | "compression"
            | "identitiesonly"
            | "ipqos"
            | "loglevel"
            | "passwordauthentication"
            | "preferredauthentications"
            | "pubkeyauthentication"
            | "requesttty"
            | "sendenv"
            | "setenv"
            | "verifyhostkeydns"
    )
}

pub fn prepare_command(
    store: &Store,
    paths: &AppPaths,
    host: &Host,
    proxy_jump: Option<&str>,
) -> Result<PreparedCommand> {
    if host.template.as_deref() == Some("mosh") {
        return Ok(PreparedCommand {
            argv: vec!["mosh".to_string(), user_at_hostname(host)],
            private_key: None,
        });
    }

    let (private_key_path, private_key) = prepare_private_key(store, paths, host)?;
    Ok(PreparedCommand {
        argv: render_ssh_argv(host, private_key_path, proxy_jump),
        private_key,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_transfer_command(
    store: &Store,
    paths: &AppPaths,
    host: &Host,
    local_path: &Path,
    remote_path: &str,
    direction: TransferDirection,
    options: TransferOptions,
    proxy_jump: Option<&str>,
) -> Result<PreparedCommand> {
    let (private_key_path, private_key) = prepare_private_key(store, paths, host)?;
    Ok(PreparedCommand {
        argv: if options.rsync {
            render_rsync_argv(
                host,
                private_key_path,
                local_path,
                remote_path,
                direction,
                options.recursive,
                proxy_jump,
            )?
        } else {
            render_scp_argv(
                host,
                private_key_path,
                local_path,
                remote_path,
                direction,
                options.recursive,
                proxy_jump,
            )?
        },
        private_key,
    })
}

fn prepare_private_key(
    store: &Store,
    paths: &AppPaths,
    host: &Host,
) -> Result<(Option<PathBuf>, Option<NamedTempFile>)> {
    if host.has_private_key {
        let blob = store
            .get_secret(&host.alias, SECRET_PRIVATE_KEY)?
            .with_context(|| format!("missing encrypted private key for {}", host.alias))?;
        let passphrase = secrets::local_key_passphrase(&paths.secret_key)?;
        let bytes = secrets::decrypt_bytes(&blob, &passphrase)?;
        let file = secrets::write_temp_private_key(&bytes, &paths.runtime_dir)?;
        let path = file.path().to_path_buf();
        Ok((Some(path), Some(file)))
    } else {
        Ok((None, None))
    }
}

pub fn user_at_hostname(host: &Host) -> String {
    match &host.user {
        Some(user) => format!("{user}@{}", host.hostname),
        None => host.hostname.clone(),
    }
}

pub struct PreparedCommand {
    pub argv: Vec<String>,
    /// Keeps any decrypted private key temp file alive only for the child command lifetime.
    private_key: Option<NamedTempFile>,
}

fn run_argv(argv: &[String]) -> Result<ExitStatus> {
    let (program, args) = argv
        .split_first()
        .context("cannot run an empty command template")?;
    Command::new(program)
        .args(args)
        .status()
        .with_context(|| format!("could not launch {program:?}"))
}

fn run_prepared(prepared: &PreparedCommand) -> Result<ExitStatus> {
    // Borrowing this field makes the lifetime requirement explicit: the
    // decrypted identity cannot be removed until the child command has exited.
    let _private_key_lifetime = &prepared.private_key;
    run_argv(&prepared.argv)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inventory::Host;
    use crate::paths::AppPaths;
    use std::path::PathBuf;

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
    fn renders_direct_ssh_argv() {
        let mut host = Host::new("prod".into(), "10.0.0.10".into());
        host.user = Some("ubuntu".into());
        host.port = Some(2222);
        host.proxy_jump = Some("bastion".into());
        host.template = Some("mosh".into());
        host.local_forwards = vec!["15432 localhost:5432".into()];
        host.options = vec!["ServerAliveInterval=60".into()];

        let argv = render_ssh_argv(&host, Some(PathBuf::from("/tmp/key")), Some("bastion"));

        assert_eq!(
            &argv[..10],
            [
                "ssh",
                "-p",
                "2222",
                "-i",
                "/tmp/key",
                "-J",
                "bastion",
                "-L",
                "15432 localhost:5432",
                "-o"
            ]
        );
        assert!(
            argv.windows(2)
                .any(|pair| pair == ["-o", "StrictHostKeyChecking=ask"])
        );
        assert!(
            argv.windows(2)
                .any(|pair| pair == ["-o", "ServerAliveInterval=15"])
        );
        assert!(!argv.iter().any(|arg| arg == "ServerAliveInterval=60"));
        assert_eq!(argv.last().unwrap(), "ubuntu@10.0.0.10");
    }

    #[test]
    fn renders_path_key_when_no_encrypted_copy() {
        let mut host = Host::new("prod".into(), "example.com".into());
        host.private_key_source_path = Some("~/.ssh/prod".into());

        let argv = render_ssh_argv(&host, None, None);

        assert_eq!(&argv[..3], ["ssh", "-i", "~/.ssh/prod"]);
        assert!(
            argv.windows(2)
                .any(|pair| pair == ["-o", "StrictHostKeyChecking=ask"])
        );
        assert_eq!(argv.last().unwrap(), "example.com");
    }

    #[test]
    fn renders_scp_send_and_receive_arguments() {
        let mut host = Host::new("prod".into(), "2001:db8::10".into());
        host.user = Some("ubuntu".into());
        host.port = Some(2222);
        host.proxy_jump = Some("bastion".into());

        let send = render_scp_argv(
            &host,
            Some(PathBuf::from("/tmp/stored-key")),
            Path::new("report.csv"),
            "/srv/renamed.csv",
            TransferDirection::Send,
            false,
            Some("bastion"),
        )
        .unwrap();
        assert_eq!(
            &send[..7],
            [
                "scp",
                "-P",
                "2222",
                "-i",
                "/tmp/stored-key",
                "-J",
                "bastion"
            ]
        );
        assert!(
            send.windows(2)
                .any(|pair| pair == ["-o", "StrictHostKeyChecking=ask"])
        );
        assert_eq!(
            &send[send.len() - 2..],
            ["report.csv", "ubuntu@[2001:db8::10]:/srv/renamed.csv"]
        );

        let receive = render_scp_argv(
            &host,
            None,
            Path::new("downloads"),
            "/var/log/app",
            TransferDirection::Receive,
            true,
            Some("bastion"),
        )
        .unwrap();
        assert!(receive.iter().any(|arg| arg == "-r"));
        assert_eq!(
            &receive[receive.len() - 2..],
            ["ubuntu@[2001:db8::10]:/var/log/app", "downloads"]
        );
    }

    #[test]
    fn renders_rsync_send_and_receive_arguments() {
        let mut host = Host::new("prod".into(), "2001:db8::10".into());
        host.user = Some("ubuntu".into());
        host.port = Some(2222);

        let send = render_rsync_argv(
            &host,
            Some(PathBuf::from("/tmp/key dir/it's key")),
            Path::new("release dir"),
            "/srv/release dir",
            TransferDirection::Send,
            false,
            Some("jump@example.com:2200"),
        )
        .unwrap();
        assert_eq!(&send[..3], ["rsync", "-avzP", "--no-recursive"]);
        assert!(send.iter().any(|arg| arg == "--protect-args"));
        let rsh = &send[send.iter().position(|arg| arg == "-e").unwrap() + 1];
        assert!(rsh.starts_with("'ssh' '-p' '2222'"));
        assert!(rsh.contains("'-i' '/tmp/key dir/it''s key'"));
        assert!(rsh.contains("'-J' 'jump@example.com:2200'"));
        assert_eq!(
            &send[send.len() - 2..],
            ["release dir", "ubuntu@[2001:db8::10]:/srv/release dir"]
        );

        let receive = render_rsync_argv(
            &host,
            None,
            Path::new("downloads"),
            "/var/log/app",
            TransferDirection::Receive,
            true,
            None,
        )
        .unwrap();
        assert!(!receive.iter().any(|arg| arg == "--no-recursive"));
        assert_eq!(
            &receive[receive.len() - 2..],
            ["ubuntu@[2001:db8::10]:/var/log/app", "downloads"]
        );
    }

    #[test]
    fn saved_jump_alias_is_used_by_ssh_scp_and_rsync() {
        let mut bastion = Host::new("bastion".into(), "2001:db8::20".into());
        bastion.user = Some("jump".into());
        bastion.port = Some(2200);
        let mut app = Host::new("app".into(), "app.example".into());
        app.proxy_jump = Some("bastion".into());
        let inventory = Inventory {
            hosts: vec![app, bastion],
            ..Inventory::default()
        };
        let app = inventory.find_host("app").unwrap();
        let proxy = diagnostics::resolve_proxy_jump(&inventory, app).unwrap();

        let ssh = render_ssh_argv(app, None, proxy.as_deref());
        assert!(
            ssh.windows(2)
                .any(|pair| pair == ["-J", "jump@[2001:db8::20]:2200"])
        );
        let scp = render_scp_argv(
            app,
            None,
            Path::new("report.csv"),
            ".",
            TransferDirection::Send,
            false,
            proxy.as_deref(),
        )
        .unwrap();
        assert!(
            scp.windows(2)
                .any(|pair| pair == ["-J", "jump@[2001:db8::20]:2200"])
        );
        let rsync = render_rsync_argv(
            app,
            None,
            Path::new("report.csv"),
            ".",
            TransferDirection::Send,
            false,
            proxy.as_deref(),
        )
        .unwrap();
        let rsh = &rsync[rsync.iter().position(|arg| arg == "-e").unwrap() + 1];
        assert!(rsh.contains("'-J' 'jump@[2001:db8::20]:2200'"));
    }

    #[test]
    fn mosh_rejects_proxy_jumps() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        let mut store = Store::open(&paths).unwrap();
        let mut host = Host::new("prod".into(), "prod.example".into());
        host.user = Some("ubuntu".into());
        host.template = Some("mosh".into());
        host.proxy_jump = Some("jump.example".into());
        let mut inventory = Inventory::default();
        inventory.hosts.push(host);
        store.save_inventory(&inventory).unwrap();

        let error = connect_alias(&inventory, &store, &paths, "prod")
            .unwrap_err()
            .to_string();
        assert!(error.contains("mosh connections do not support proxy jumps"));
    }

    #[test]
    fn rejects_invalid_transfer_paths() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing");
        assert!(validate_send_source(&missing, false).is_err());
        assert!(validate_send_source(dir.path(), false).is_err());
        assert!(validate_send_source(dir.path(), true).is_ok());

        let host = Host::new("prod".into(), "example.com".into());
        assert!(
            render_scp_argv(
                &host,
                None,
                Path::new("file"),
                "bad\npath",
                TransferDirection::Send,
                false,
                None,
            )
            .is_err()
        );
        assert!(
            render_rsync_argv(
                &host,
                None,
                Path::new("file"),
                "bad\npath",
                TransferDirection::Send,
                false,
                None,
            )
            .is_err()
        );
        assert!(render_rsync_rsh(&["ssh".into(), "bad\nargument".into()]).is_err());
    }

    #[test]
    fn stored_identity_lives_for_the_prepared_scp_command() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths(dir.path());
        let mut store = Store::open(&paths).unwrap();
        let mut inventory = Inventory::default();
        let mut host = Host::new("prod".into(), "example.com".into());
        host.user = Some("ubuntu".into());
        inventory.hosts.push(host);
        store.save_inventory(&inventory).unwrap();

        let passphrase = secrets::local_key_passphrase(&paths.secret_key).unwrap();
        let blob = secrets::encrypt_bytes(b"stored-private-key", &passphrase, None).unwrap();
        store.put_secret("prod", SECRET_PRIVATE_KEY, &blob).unwrap();
        let inventory = store.load_inventory().unwrap();
        let prepared = prepare_transfer_command(
            &store,
            &paths,
            &inventory.hosts[0],
            Path::new("report.csv"),
            ".",
            TransferDirection::Send,
            TransferOptions {
                recursive: false,
                rsync: false,
            },
            None,
        )
        .unwrap();
        let key_path = PathBuf::from(
            prepared
                .argv
                .windows(2)
                .find(|pair| pair[0] == "-i")
                .unwrap()[1]
                .clone(),
        );
        assert!(key_path.exists());
        drop(prepared);
        assert!(!key_path.exists());
    }

    #[test]
    fn temp_private_key_is_deleted_after_launch_error() {
        let dir = tempfile::tempdir().unwrap();
        let runtime_dir = dir.path().join("runtime");
        let private_key = secrets::write_temp_private_key(b"not-a-real-key", &runtime_dir).unwrap();
        let key_path = private_key.path().to_path_buf();
        assert!(key_path.exists());
        let prepared = PreparedCommand {
            argv: vec!["/definitely/not/a/real/sshnav-command".to_string()],
            private_key: Some(private_key),
        };

        let result = run_argv(&prepared.argv);
        assert!(result.is_err());
        drop(prepared);

        assert!(!key_path.exists());
    }

    #[test]
    fn reconnect_is_limited_to_confirmed_transport_losses() {
        assert!(should_reconnect(255, true, true, 0));
        assert!(should_reconnect(255, true, true, 2));
        assert!(!should_reconnect(255, true, true, 3));
        assert!(!should_reconnect(255, true, false, 0));
        assert!(!should_reconnect(1, true, true, 0));
        assert!(!should_reconnect(255, false, true, 0));
    }
}
