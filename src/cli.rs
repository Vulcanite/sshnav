use crate::add_form::{self, AddHostDraft, AuthChoice, EditHostAction};
use crate::diagnostics;
use crate::doctor;
use crate::generator;
use crate::inventory::{Host, Inventory};
use crate::paths::AppPaths;
use crate::picker;
use crate::runner;
use crate::secrets;
use crate::ssh_config;
use crate::storage::{SECRET_PRIVATE_KEY, Store};
use crate::term;
use anyhow::{Context, Result, bail};
use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use fuzzy_matcher::FuzzyMatcher;
use fuzzy_matcher::skim::SkimMatcherV2;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const COMMANDS: &[&str] = &[
    "add",
    "pick",
    "connect",
    "send",
    "receive",
    "host",
    "import",
    "generate",
    "doctor",
    "migrate",
    "completions",
];

#[derive(Debug, Parser)]
#[command(
    name = "sshnav",
    version,
    about = "A fast local SSH inventory navigator and launcher"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
enum Command {
    /// Open the interactive add-host form.
    Add,
    /// Open the searchable picker.
    Pick(PickArgs),
    /// Connect directly to a host alias.
    Connect(ConnectArgs),
    /// Copy a local file or directory to a host.
    Send(SendArgs),
    /// Copy a remote file or directory from a host.
    Receive(ReceiveArgs),
    /// Manage hosts in the sshnav database.
    #[command(subcommand)]
    Host(HostCommand),
    /// Import hosts from another source.
    #[command(subcommand)]
    Import(ImportCommand),
    /// Generate the optional managed OpenSSH include file.
    Generate(GenerateArgs),
    /// Migrate hosts from an existing OpenSSH config into SQLite.
    Migrate(MigrateArgs),
    /// Check environment health or diagnose a host chain.
    Doctor(DoctorArgs),
    /// Generate shell completion code.
    Completions(CompletionsArgs),
}

#[derive(Debug, Args)]
struct PickArgs {
    query: Option<String>,
}

#[derive(Debug, Args)]
struct ConnectArgs {
    alias: String,
}

#[derive(Debug, Args)]
#[command(override_usage = "sshnav send <ALIAS> <LOCAL_SOURCE> [REMOTE_DESTINATION] [OPTIONS]")]
struct SendArgs {
    /// Saved sshnav host alias.
    alias: String,
    /// Local file or directory to copy.
    local_source: PathBuf,
    /// Remote directory or file path.
    #[arg(default_value = ".")]
    remote_destination: String,
    /// Copy directories recursively.
    #[arg(short = 'r', long)]
    recursive: bool,
    /// Use rsync with archive, compression, partial, and progress support.
    #[arg(long)]
    rsync: bool,
}

#[derive(Debug, Args)]
#[command(override_usage = "sshnav receive <ALIAS> <REMOTE_SOURCE> [LOCAL_DESTINATION] [OPTIONS]")]
struct ReceiveArgs {
    /// Saved sshnav host alias.
    alias: String,
    /// Remote file or directory to copy.
    remote_source: String,
    /// Local directory or file path.
    #[arg(default_value = ".")]
    local_destination: PathBuf,
    /// Copy directories recursively.
    #[arg(short = 'r', long)]
    recursive: bool,
    /// Use rsync with archive, compression, partial, and progress support.
    #[arg(long)]
    rsync: bool,
}

#[derive(Debug, Args)]
struct DoctorArgs {
    alias: Option<String>,
}

#[derive(Debug, Args)]
struct CompletionsArgs {
    /// Shell to generate completion code for.
    #[arg(value_enum)]
    shell: Shell,
}

#[derive(Debug, Subcommand)]
enum HostCommand {
    List(ListArgs),
    Add(AddArgs),
    Edit(EditArgs),
    /// Copy a saved host to a new alias.
    Duplicate(DuplicateArgs),
    Remove(RemoveArgs),
    #[command(name = "update-key")]
    UpdateKey(UpdateKeyArgs),
    #[command(name = "remove-key")]
    RemoveKey(HostAliasArgs),
    #[command(name = "forget-key-source")]
    ForgetKeySource(HostAliasArgs),
}

#[derive(Debug, Args)]
struct ListArgs {
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct AddArgs {
    #[arg(long)]
    alias: String,
    #[arg(long)]
    hostname: String,
    #[arg(long, required = true)]
    user: String,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long)]
    display_name: Option<String>,
    #[arg(long)]
    group: Option<String>,
    #[arg(long)]
    identity_file: Option<PathBuf>,
    #[arg(long)]
    template: Option<String>,
    #[arg(long)]
    proxy_jump: Option<String>,
    #[arg(long)]
    auto_reconnect: bool,
}

#[derive(Debug, Args)]
struct EditArgs {
    alias: String,
    #[arg(long)]
    hostname: Option<String>,
    #[arg(long)]
    user: Option<String>,
    #[arg(long)]
    port: Option<u16>,
    #[arg(long = "tag")]
    tags: Vec<String>,
    #[arg(long)]
    display_name: Option<String>,
    #[arg(long)]
    group: Option<String>,
    #[arg(long)]
    identity_file: Option<PathBuf>,
    #[arg(long)]
    template: Option<String>,
    #[arg(long, conflicts_with = "no_proxy_jump")]
    proxy_jump: Option<String>,
    /// Clear the saved proxy jump.
    #[arg(long, conflicts_with = "proxy_jump")]
    no_proxy_jump: bool,
    #[arg(long, conflicts_with = "no_auto_reconnect")]
    auto_reconnect: bool,
    #[arg(long, conflicts_with = "auto_reconnect")]
    no_auto_reconnect: bool,
}

#[derive(Debug, Args)]
struct DuplicateArgs {
    /// Existing host alias to copy.
    source_alias: String,
    /// New, unique host alias.
    new_alias: String,
}

#[derive(Debug, Args)]
struct RemoveArgs {
    alias: String,
}

#[derive(Debug, Args)]
struct UpdateKeyArgs {
    alias: String,
    #[arg(long = "from")]
    from: PathBuf,
}

#[derive(Debug, Args)]
struct HostAliasArgs {
    alias: String,
}

#[derive(Debug, Subcommand)]
enum ImportCommand {
    #[command(name = "ssh-config")]
    SshConfig(ImportSshConfigArgs),
}

#[derive(Debug, Args)]
struct ImportSshConfigArgs {
    #[arg(long = "from")]
    from: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct GenerateArgs {
    #[arg(long)]
    install_include: bool,
    #[arg(long)]
    yes: bool,
}

#[derive(Debug, Args)]
struct MigrateArgs {
    #[arg(long = "from")]
    from: Option<PathBuf>,
}

pub fn run() -> Result<i32> {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(err) => return handle_parse_error(err),
    };
    if let Some(Command::Completions(args)) = &cli.command {
        generate(args.shell, &mut Cli::command(), "sshnav", &mut io::stdout());
        return Ok(0);
    }
    let paths = AppPaths::discover()?;
    let mut store = Store::open(&paths)?;

    match cli.command {
        None => run_picker_loop(&paths, &mut store, None),
        Some(Command::Add) => run_interactive_add(&paths, &mut store),
        Some(Command::Pick(args)) => run_picker_loop(&paths, &mut store, args.query),
        Some(Command::Connect(args)) => run_connect(&paths, &store, &args.alias),
        Some(Command::Send(args)) => run_send(&paths, &store, args),
        Some(Command::Receive(args)) => run_receive(&paths, &store, args),
        Some(Command::Host(command)) => run_host_command(&paths, &mut store, command),
        Some(Command::Import(command)) => run_import_command(&paths, &mut store, command),
        Some(Command::Generate(args)) => run_generate(&paths, &store, args),
        Some(Command::Migrate(args)) => run_migrate_command(&paths, &mut store, args),
        Some(Command::Doctor(args)) => run_doctor(&paths, &store, args),
        Some(Command::Completions(_)) => unreachable!(),
    }
}

fn run_picker_loop(
    paths: &AppPaths,
    store: &mut Store,
    initial_query: Option<String>,
) -> Result<i32> {
    let mut query = initial_query;
    loop {
        let inventory = store.load_inventory()?;
        let Some(action) = picker::select_host(&inventory, query.take(), &paths.db)? else {
            return Ok(0);
        };
        match action {
            picker::PickerAction::Add => {
                run_interactive_add(paths, store)?;
            }
            picker::PickerAction::Connect(alias) => {
                let code = runner::connect_alias(&inventory, store, paths, &alias)?;
                if code != 0 {
                    term::error(format!("sshnav: session for {alias} exited with {code}"));
                }
            }
            picker::PickerAction::Edit(alias) => {
                run_interactive_edit(paths, store, &alias)?;
            }
            picker::PickerAction::Duplicate(alias) => {
                run_interactive_duplicate(paths, store, &alias)?;
            }
        }
    }
}

fn run_connect(paths: &AppPaths, store: &Store, alias: &str) -> Result<i32> {
    let inventory = store.load_inventory()?;
    if report_unknown_alias(&inventory, alias, "connect") {
        return Ok(1);
    }
    runner::connect_alias(&inventory, store, paths, alias)
}

fn run_send(paths: &AppPaths, store: &Store, args: SendArgs) -> Result<i32> {
    let inventory = store.load_inventory()?;
    if report_unknown_alias(&inventory, &args.alias, "send") {
        return Ok(1);
    }
    runner::send_alias(
        &inventory,
        store,
        paths,
        &args.alias,
        &args.local_source,
        &args.remote_destination,
        runner::TransferOptions {
            recursive: args.recursive,
            rsync: args.rsync,
        },
    )
}

fn run_receive(paths: &AppPaths, store: &Store, args: ReceiveArgs) -> Result<i32> {
    let inventory = store.load_inventory()?;
    if report_unknown_alias(&inventory, &args.alias, "receive") {
        return Ok(1);
    }
    runner::receive_alias(
        &inventory,
        store,
        paths,
        &args.alias,
        &args.remote_source,
        &args.local_destination,
        runner::TransferOptions {
            recursive: args.recursive,
            rsync: args.rsync,
        },
    )
}

fn report_unknown_alias(inventory: &Inventory, alias: &str, command: &str) -> bool {
    if inventory.find_host(alias).is_some() {
        return false;
    }
    term::error(format!("unknown host alias {alias:?}"));
    if let Some(suggestion) = suggest_host(inventory, alias) {
        eprintln!("tip: did you mean `sshnav {command} {suggestion}`?");
    }
    true
}

fn run_doctor(paths: &AppPaths, store: &Store, args: DoctorArgs) -> Result<i32> {
    if let Some(alias) = args.alias {
        let inventory = store.load_inventory()?;
        diagnostics::print_alias_diagnostics(&inventory, &alias)
    } else {
        doctor::run_and_print(paths, store)
    }
}

fn run_generate(paths: &AppPaths, store: &Store, args: GenerateArgs) -> Result<i32> {
    let inventory = store.load_inventory()?;
    generator::write_generated(&inventory, &paths.generated_config)?;
    if args.install_include {
        if args.yes
            || confirm(&format!(
                "Install sshnav Include line into {}?",
                paths.ssh_config.display()
            ))?
        {
            let changed = generator::install_include(&paths.ssh_config)?;
            if changed {
                term::success(format!(
                    "installed include in {}",
                    paths.ssh_config.display()
                ));
            } else {
                println!(
                    "include already installed in {}",
                    paths.ssh_config.display()
                );
            }
        } else {
            println!("skipped editing {}", paths.ssh_config.display());
        }
    }
    term::success(format!("wrote {}", paths.generated_config.display()));
    Ok(0)
}

fn run_interactive_add(paths: &AppPaths, store: &mut Store) -> Result<i32> {
    let inventory = store.load_inventory()?;
    let mut groups = inventory
        .hosts
        .iter()
        .filter_map(|host| host.group.clone())
        .collect::<Vec<_>>();
    groups.sort();
    groups.dedup();
    let aliases = inventory
        .hosts
        .iter()
        .map(|host| host.alias.clone())
        .collect::<Vec<_>>();

    let Some(draft) = add_form::open(&groups, &aliases)? else {
        return Ok(0);
    };

    add_host_from_draft(paths, store, draft)
}

fn run_interactive_edit(paths: &AppPaths, store: &mut Store, alias: &str) -> Result<i32> {
    let inventory = store.load_inventory()?;
    let mut groups = inventory
        .hosts
        .iter()
        .filter_map(|host| host.group.clone())
        .collect::<Vec<_>>();
    groups.sort();
    groups.dedup();
    let aliases = inventory
        .hosts
        .iter()
        .map(|host| host.alias.clone())
        .collect::<Vec<_>>();

    let host = inventory
        .find_host(alias)
        .with_context(|| unknown_alias_message(&inventory, alias))?
        .clone();
    let Some(action) = add_form::open_edit(&groups, &aliases, &host)? else {
        return Ok(0);
    };

    match action {
        EditHostAction::Save(draft) => edit_host_from_draft(paths, store, &host, draft),
        EditHostAction::Delete => remove_host(store, alias),
    }
}

fn run_interactive_duplicate(paths: &AppPaths, store: &mut Store, alias: &str) -> Result<i32> {
    let inventory = store.load_inventory()?;
    let source = inventory
        .find_host(alias)
        .with_context(|| unknown_alias_message(&inventory, alias))?
        .clone();
    let mut groups = inventory
        .hosts
        .iter()
        .filter_map(|host| host.group.clone())
        .collect::<Vec<_>>();
    groups.sort();
    groups.dedup();
    let aliases = inventory
        .hosts
        .iter()
        .map(|host| host.alias.clone())
        .collect::<Vec<_>>();
    let suggested = unique_duplicate_alias(&inventory, alias);
    let Some(draft) = add_form::open_duplicate(&groups, &aliases, &source, &suggested)? else {
        return Ok(0);
    };
    duplicate_host_from_draft(paths, store, &source, draft)
}

fn add_host_from_draft(paths: &AppPaths, store: &mut Store, draft: AddHostDraft) -> Result<i32> {
    let mut inventory = store.load_inventory()?;
    if inventory.find_host(&draft.alias).is_some() {
        bail!("host alias {:?} already exists", draft.alias);
    }
    let resolved_private_key = draft
        .private_key
        .as_ref()
        .map(|path| secrets::resolve_private_key_path(path))
        .transpose()?;

    let mut host = Host::new(draft.alias.clone(), draft.hostname);
    host.user = draft.user;
    host.port = draft.port;
    host.tags = draft.tags;
    host.group = draft.group;
    host.proxy_jump = draft.proxy_jump;
    host.private_key_source_path = resolved_private_key
        .as_ref()
        .map(|path| path.display().to_string());
    host.auto_reconnect = draft.auto_reconnect;
    inventory.hosts.push(host);
    inventory.hosts.sort_by(|a, b| a.alias.cmp(&b.alias));
    store.save_inventory(&inventory)?;

    match draft.auth {
        AuthChoice::OpenSshDefault => {}
        AuthChoice::PrivateKey => {
            let path = resolved_private_key
                .context("private key path is required for private key auth")?;
            import_private_key(paths, store, &draft.alias, &path)?;
        }
    }

    term::success(format!("added host {}", draft.alias));
    Ok(0)
}

fn edit_host_from_draft(
    paths: &AppPaths,
    store: &mut Store,
    original: &Host,
    draft: AddHostDraft,
) -> Result<i32> {
    if draft.alias != original.alias {
        bail!("interactive edit cannot rename host aliases yet");
    }

    let resolved_private_key = if draft.auth == AuthChoice::PrivateKey {
        draft
            .private_key
            .as_deref()
            .map(secrets::resolve_private_key_path)
            .transpose()?
    } else {
        None
    };
    let private_key_changed = resolved_private_key
        .as_ref()
        .map(|path| path.display().to_string())
        .as_deref()
        != original.private_key_source_path.as_deref();
    if draft.auth == AuthChoice::PrivateKey
        && resolved_private_key.is_none()
        && !original.has_private_key
    {
        bail!("private key path is required for private key auth");
    }
    if draft.auth == AuthChoice::PrivateKey
        && (!original.has_private_key || private_key_changed)
        && let Some(path) = resolved_private_key.as_ref()
    {
        secrets::validate_private_key(path)?;
    }

    let mut inventory = store.load_inventory()?;
    let missing_message = unknown_alias_message(&inventory, &draft.alias);
    let host = inventory
        .find_host_mut(&draft.alias)
        .with_context(|| missing_message.clone())?;
    host.hostname = draft.hostname;
    host.user = draft.user;
    host.port = draft.port;
    host.tags = draft.tags;
    host.group = draft.group;
    host.proxy_jump = draft.proxy_jump;
    host.auto_reconnect = draft.auto_reconnect;
    host.private_key_source_path = if draft.auth == AuthChoice::PrivateKey {
        resolved_private_key
            .as_ref()
            .map(|path| path.display().to_string())
            .or_else(|| original.private_key_source_path.clone())
    } else {
        None
    };
    store.save_inventory(&inventory)?;

    match draft.auth {
        AuthChoice::OpenSshDefault => {
            store.remove_secret(&draft.alias, SECRET_PRIVATE_KEY)?;
        }
        AuthChoice::PrivateKey => {
            if let Some(path) = resolved_private_key {
                let path_text = path.display().to_string();
                if !original.has_private_key
                    || original.private_key_source_path.as_deref() != Some(&path_text)
                {
                    import_private_key(paths, store, &draft.alias, &path)?;
                }
            } else if !original.has_private_key {
                bail!("private key path is required for private key auth");
            }
        }
    }

    term::success(format!("updated host {}", draft.alias));
    Ok(0)
}

fn run_host_command(_paths: &AppPaths, store: &mut Store, command: HostCommand) -> Result<i32> {
    let mut inventory = store.load_inventory()?;
    match command {
        HostCommand::List(args) => {
            if args.json {
                println!("{}", serde_json::to_string_pretty(&inventory.hosts)?);
            } else {
                print_host_list(&inventory);
            }
            Ok(0)
        }
        HostCommand::Add(args) => {
            if inventory.find_host(&args.alias).is_some() {
                bail!("host alias {:?} already exists", args.alias);
            }
            let resolved_identity_file = args
                .identity_file
                .as_ref()
                .map(|path| secrets::resolve_private_key_path(path))
                .transpose()?;
            let mut host = Host::new(args.alias.clone(), args.hostname);
            host.user = Some(args.user);
            host.port = args.port;
            host.tags = normalize_tags(args.tags);
            host.display_name = args.display_name;
            host.group = args.group;
            host.private_key_source_path = resolved_identity_file
                .as_ref()
                .map(|path| path.display().to_string());
            host.template = args.template;
            host.proxy_jump = args.proxy_jump;
            host.auto_reconnect = args.auto_reconnect;
            inventory.hosts.push(host);
            inventory.hosts.sort_by(|a, b| a.alias.cmp(&b.alias));
            store.save_inventory(&inventory)?;
            if let Some(path) = resolved_identity_file {
                import_private_key(_paths, store, &args.alias, &path)?;
            }
            term::success("added host");
            Ok(0)
        }
        HostCommand::Edit(args) => {
            let resolved_identity_file = if let Some(path) = args.identity_file.as_ref() {
                Some(secrets::resolve_private_key_path(path)?)
            } else {
                None
            };
            let missing_message = unknown_alias_message(&inventory, &args.alias);
            let host = inventory
                .find_host_mut(&args.alias)
                .with_context(|| missing_message.clone())?;
            if let Some(value) = args.hostname {
                host.hostname = value;
            }
            if let Some(value) = args.user {
                host.user = Some(value);
            }
            if let Some(value) = args.port {
                host.port = Some(value);
            }
            if !args.tags.is_empty() {
                host.tags = normalize_tags(args.tags);
            }
            if let Some(value) = args.display_name {
                host.display_name = Some(value);
            }
            if let Some(value) = args.group {
                host.group = Some(value);
            }
            if let Some(value) = resolved_identity_file.as_ref() {
                host.private_key_source_path = Some(value.display().to_string());
            }
            if let Some(value) = args.template {
                host.template = Some(value);
            }
            if let Some(value) = args.proxy_jump {
                host.proxy_jump = Some(value);
            }
            if args.no_proxy_jump {
                host.proxy_jump = None;
            }
            if args.auto_reconnect {
                host.auto_reconnect = true;
            }
            if args.no_auto_reconnect {
                host.auto_reconnect = false;
            }
            store.save_inventory(&inventory)?;
            if let Some(path) = resolved_identity_file {
                import_private_key(_paths, store, &args.alias, &path)?;
            }
            term::success("updated host");
            Ok(0)
        }
        HostCommand::Duplicate(args) => duplicate_host(store, &args.source_alias, &args.new_alias),
        HostCommand::Remove(args) => remove_host(store, &args.alias),
        HostCommand::UpdateKey(args) => {
            let resolved_key = secrets::resolve_private_key_path(&args.from)?;
            let missing_message = unknown_alias_message(&inventory, &args.alias);
            let host = inventory
                .find_host_mut(&args.alias)
                .with_context(|| missing_message.clone())?;
            host.private_key_source_path = Some(resolved_key.display().to_string());
            store.save_inventory(&inventory)?;
            import_private_key(_paths, store, &args.alias, &resolved_key)?;
            term::success(format!("updated encrypted private key for {}", args.alias));
            Ok(0)
        }
        HostCommand::RemoveKey(args) => {
            let missing_message = unknown_alias_message(&inventory, &args.alias);
            let host = inventory
                .find_host_mut(&args.alias)
                .with_context(|| missing_message.clone())?;
            host.private_key_source_path = None;
            store.save_inventory(&inventory)?;
            if store.remove_secret(&args.alias, SECRET_PRIVATE_KEY)? {
                term::success(format!("removed private key for {}", args.alias));
            } else {
                println!("no private key stored for {}", args.alias);
            }
            Ok(0)
        }
        HostCommand::ForgetKeySource(args) => {
            let missing_message = unknown_alias_message(&inventory, &args.alias);
            let host = inventory
                .find_host_mut(&args.alias)
                .with_context(|| missing_message.clone())?;
            if !store.has_secret(&args.alias, SECRET_PRIVATE_KEY)? {
                bail!(
                    "host {:?} has no encrypted private key; refusing to remove its fallback source path",
                    args.alias
                );
            }
            host.private_key_source_path = None;
            store.save_inventory(&inventory)?;
            term::success(format!("forgot private key source path for {}", args.alias));
            Ok(0)
        }
    }
}

fn run_import_command(paths: &AppPaths, store: &mut Store, command: ImportCommand) -> Result<i32> {
    match command {
        ImportCommand::SshConfig(args) => import_ssh_config(paths, store, args.from, "imported"),
    }
}

fn run_migrate_command(paths: &AppPaths, store: &mut Store, args: MigrateArgs) -> Result<i32> {
    import_ssh_config(paths, store, args.from, "migrated")
}

fn import_ssh_config(
    paths: &AppPaths,
    store: &mut Store,
    from: Option<PathBuf>,
    verb: &str,
) -> Result<i32> {
    let source = from
        .map(|path| secrets::expand_tilde(&path))
        .unwrap_or_else(|| paths.ssh_config.clone());
    let import_result = ssh_config::import_file_with_warnings(&source)?;
    let placeholder_user_count = import_result.placeholder_user_count;
    let imported = import_result.hosts;
    let imported_aliases = imported
        .iter()
        .map(|host| host.alias.clone())
        .collect::<Vec<_>>();
    let count = imported.len();
    let mut inventory = store.load_inventory()?;
    inventory.upsert_imported_hosts(imported);
    store.save_inventory(&inventory)?;

    let refreshed = store.load_inventory()?;
    for host in refreshed.hosts.iter().filter(|host| {
        imported_aliases.contains(&host.alias)
            && host.private_key_source_path.is_some()
            && !host.has_private_key
    }) {
        let Some(path) = host.private_key_source_path.as_ref().map(PathBuf::from) else {
            continue;
        };
        if confirm(&format!(
            "Import private key for {} from {}?",
            host.alias,
            path.display()
        ))? {
            import_private_key(paths, store, &host.alias, &path)?;
        }
    }
    term::success(format!("{verb} {count} host(s) from {}", source.display()));
    if placeholder_user_count > 0 {
        term::warn(format!(
            "{placeholder_user_count} imported host(s) used placeholder username \"user\" because no OS username could be detected; edit them before connecting"
        ));
    }
    Ok(0)
}

fn import_private_key(paths: &AppPaths, store: &Store, alias: &str, path: &Path) -> Result<()> {
    let passphrase = secrets::local_key_passphrase(&paths.secret_key)?;
    let blob = secrets::encrypt_file(path, &passphrase)?;
    store.put_secret(alias, SECRET_PRIVATE_KEY, &blob)?;
    Ok(())
}

fn print_host_list(inventory: &Inventory) {
    for host in &inventory.hosts {
        let user = host.user.as_deref().unwrap_or("-");
        let port = host
            .port
            .map(|port| port.to_string())
            .unwrap_or_else(|| "-".to_string());
        let tags = if host.tags.is_empty() {
            "-".to_string()
        } else {
            host.tags.join(",")
        };
        let secrets = if host.has_private_key { "key" } else { "-" };
        println!(
            "{:<24} {:<32} user={:<16} port={:<6} tags={} secrets={}",
            host.alias, host.hostname, user, port, tags, secrets
        );
    }
}

fn duplicate_host(store: &mut Store, source_alias: &str, new_alias: &str) -> Result<i32> {
    let mut inventory = store.load_inventory()?;
    if inventory.find_host(new_alias).is_some() {
        bail!("host alias {new_alias:?} already exists");
    }
    let mut duplicate = inventory
        .find_host(source_alias)
        .with_context(|| unknown_alias_message(&inventory, source_alias))?
        .clone();
    let key = if duplicate.has_private_key {
        Some(
            store
                .get_secret(source_alias, SECRET_PRIVATE_KEY)?
                .with_context(|| format!("missing encrypted private key for {source_alias}"))?,
        )
    } else {
        None
    };
    duplicate.alias = new_alias.to_string();
    inventory.hosts.push(duplicate);
    inventory.hosts.sort_by(|a, b| a.alias.cmp(&b.alias));
    store.save_inventory(&inventory)?;
    if let Some(key) = key {
        store.put_secret(new_alias, SECRET_PRIVATE_KEY, &key)?;
    }
    term::success(format!("duplicated host {source_alias} as {new_alias}"));
    Ok(0)
}

fn duplicate_host_from_draft(
    paths: &AppPaths,
    store: &mut Store,
    source: &Host,
    draft: AddHostDraft,
) -> Result<i32> {
    let mut inventory = store.load_inventory()?;
    if inventory.find_host(&draft.alias).is_some() {
        bail!("host alias {:?} already exists", draft.alias);
    }
    let resolved_key = draft
        .private_key
        .as_deref()
        .map(secrets::resolve_private_key_path)
        .transpose()?;
    if let Some(path) = &resolved_key {
        secrets::validate_private_key(path)?;
    }
    let copied_key = if draft.auth == AuthChoice::PrivateKey
        && resolved_key.is_none()
        && source.has_private_key
    {
        Some(
            store
                .get_secret(&source.alias, SECRET_PRIVATE_KEY)?
                .with_context(|| format!("missing encrypted private key for {}", source.alias))?,
        )
    } else {
        None
    };

    let mut duplicate = source.clone();
    duplicate.alias = draft.alias.clone();
    duplicate.hostname = draft.hostname;
    duplicate.user = draft.user;
    duplicate.port = draft.port;
    duplicate.group = draft.group;
    duplicate.tags = draft.tags;
    duplicate.proxy_jump = draft.proxy_jump;
    duplicate.auto_reconnect = draft.auto_reconnect;
    duplicate.private_key_source_path = match draft.auth {
        AuthChoice::OpenSshDefault => None,
        AuthChoice::PrivateKey => resolved_key
            .as_ref()
            .map(|path| path.display().to_string())
            .or_else(|| source.private_key_source_path.clone()),
    };
    inventory.hosts.push(duplicate);
    inventory.hosts.sort_by(|a, b| a.alias.cmp(&b.alias));
    store.save_inventory(&inventory)?;

    if let Some(path) = resolved_key {
        import_private_key(paths, store, &draft.alias, &path)?;
    } else if let Some(key) = copied_key {
        store.put_secret(&draft.alias, SECRET_PRIVATE_KEY, &key)?;
    }
    term::success(format!(
        "duplicated host {} as {}",
        source.alias, draft.alias
    ));
    Ok(0)
}

fn unique_duplicate_alias(inventory: &Inventory, alias: &str) -> String {
    let base = format!("{alias}-copy");
    if inventory.find_host(&base).is_none() {
        return base;
    }
    (2..)
        .map(|number| format!("{base}-{number}"))
        .find(|candidate| inventory.find_host(candidate).is_none())
        .expect("an unused duplicate alias")
}

fn remove_host(store: &mut Store, alias: &str) -> Result<i32> {
    let mut inventory = store.load_inventory()?;
    let original_len = inventory.hosts.len();
    inventory.hosts.retain(|host| host.alias != alias);
    if inventory.hosts.len() == original_len {
        bail!("{}", unknown_alias_message(&inventory, alias));
    }
    store.save_inventory(&inventory)?;
    term::success(format!("removed host {alias}"));
    Ok(0)
}

fn normalize_tags(tags: Vec<String>) -> Vec<String> {
    tags.into_iter()
        .flat_map(|tag| {
            tag.split(',')
                .map(str::trim)
                .filter(|tag| !tag.is_empty())
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .collect()
}

fn unknown_alias_message(inventory: &Inventory, alias: &str) -> String {
    match suggest_host(inventory, alias) {
        Some(suggestion) => {
            format!("unknown host alias {alias:?}; tip: did you mean {suggestion:?}?")
        }
        None => format!("unknown host alias {alias:?}"),
    }
}

fn suggest_host(inventory: &Inventory, alias: &str) -> Option<String> {
    let matcher = SkimMatcherV2::default();
    let fuzzy = inventory
        .hosts
        .iter()
        .filter_map(|host| {
            matcher
                .fuzzy_match(&host.alias, alias)
                .map(|score| (score, host.alias.clone()))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, alias)| alias);
    fuzzy.or_else(|| {
        inventory
            .hosts
            .iter()
            .map(|host| (edit_distance(&host.alias, alias), host.alias.clone()))
            .filter(|(distance, _)| *distance <= 3)
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, alias)| alias)
    })
}

fn handle_parse_error(err: clap::Error) -> Result<i32> {
    let kind = err.kind();
    if matches!(kind, ErrorKind::DisplayHelp | ErrorKind::DisplayVersion) {
        let _ = err.print();
        return Ok(0);
    }
    term::error(err);
    if matches!(
        kind,
        ErrorKind::InvalidSubcommand | ErrorKind::UnknownArgument
    ) && let Some(input) = std::env::args().nth(1)
        && let Some(suggestion) = suggest_command(&input)
    {
        eprintln!("tip: did you mean `sshnav {suggestion}`?");
    }
    Ok(2)
}

fn suggest_command(input: &str) -> Option<&'static str> {
    let matcher = SkimMatcherV2::default();
    let fuzzy = COMMANDS
        .iter()
        .filter_map(|command| {
            matcher
                .fuzzy_match(command, input)
                .map(|score| (score, *command))
        })
        .max_by_key(|(score, _)| *score)
        .map(|(_, command)| command);
    fuzzy.or_else(|| {
        COMMANDS
            .iter()
            .map(|command| (edit_distance(command, input), *command))
            .filter(|(distance, _)| *distance <= 3)
            .min_by_key(|(distance, _)| *distance)
            .map(|(_, command)| command)
    })
}

fn edit_distance(left: &str, right: &str) -> usize {
    let right_chars = right.chars().collect::<Vec<_>>();
    let mut previous = (0..=right_chars.len()).collect::<Vec<_>>();
    for (i, left_ch) in left.chars().enumerate() {
        let mut current = vec![i + 1];
        for (j, right_ch) in right_chars.iter().enumerate() {
            let substitution = usize::from(left_ch != *right_ch);
            current.push(
                (previous[j + 1] + 1)
                    .min(current[j] + 1)
                    .min(previous[j] + substitution),
            );
        }
        previous = current;
    }
    previous[right_chars.len()]
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt} [y/N] ");
    io::stdout().flush()?;
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(matches!(input.trim(), "y" | "Y" | "yes" | "YES"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::SecretBlob;
    use clap::CommandFactory;

    fn test_paths(root: &Path) -> AppPaths {
        AppPaths {
            db: root.join("data/sshnav.db"),
            secret_key: root.join("data/secret.key"),
            runtime_dir: root.join("runtime"),
            generated_config: root.join(".ssh/sshnav.generated"),
            ssh_config: root.join(".ssh/config"),
        }
    }

    #[test]
    fn normalizes_repeated_and_comma_separated_tags() {
        assert_eq!(
            normalize_tags(vec!["prod, db".into(), "oracle".into(), " ".into()]),
            vec!["prod", "db", "oracle"]
        );
    }

    #[test]
    fn parses_transfer_defaults_and_recursive_destinations() {
        let send = Cli::try_parse_from(["sshnav", "send", "prod", "report.csv"]).unwrap();
        let Some(Command::Send(args)) = send.command else {
            panic!("expected send command");
        };
        assert_eq!(args.remote_destination, ".");
        assert!(!args.recursive);
        assert!(!args.rsync);

        let receive_default =
            Cli::try_parse_from(["sshnav", "receive", "prod", "/var/log/app.log"]).unwrap();
        let Some(Command::Receive(args)) = receive_default.command else {
            panic!("expected receive command");
        };
        assert_eq!(args.local_destination, PathBuf::from("."));
        assert!(!args.recursive);
        assert!(!args.rsync);

        let receive = Cli::try_parse_from([
            "sshnav",
            "receive",
            "prod",
            "/var/log/app",
            "downloads",
            "--recursive",
        ])
        .unwrap();
        let Some(Command::Receive(args)) = receive.command else {
            panic!("expected receive command");
        };
        assert_eq!(args.local_destination, PathBuf::from("downloads"));
        assert!(args.recursive);

        let send_recursive =
            Cli::try_parse_from(["sshnav", "send", "prod", "assets", "/srv/app", "-r"]).unwrap();
        let Some(Command::Send(args)) = send_recursive.command else {
            panic!("expected send command");
        };
        assert!(args.recursive);

        let rsync = Cli::try_parse_from([
            "sshnav",
            "send",
            "prod",
            "report.csv",
            "/srv/report.csv",
            "--rsync",
        ])
        .unwrap();
        let Some(Command::Send(args)) = rsync.command else {
            panic!("expected send command");
        };
        assert!(args.rsync);
    }

    #[test]
    fn parses_completion_shell() {
        let cli = Cli::try_parse_from(["sshnav", "completions", "bash"]).unwrap();
        let Some(Command::Completions(args)) = cli.command else {
            panic!("expected completions command");
        };
        assert_eq!(args.shell, Shell::Bash);
    }

    #[test]
    fn transfer_help_describes_arguments_and_places_options_last() {
        let mut command = Cli::command();
        let help = command
            .find_subcommand_mut("send")
            .unwrap()
            .render_help()
            .to_string();

        assert!(
            help.contains(
                "Usage: sshnav send <ALIAS> <LOCAL_SOURCE> [REMOTE_DESTINATION] [OPTIONS]"
            )
        );
        assert!(help.contains("Saved sshnav host alias"));
        assert!(help.contains("Local file or directory to copy"));
        assert!(help.contains("-r, --recursive"));
        assert!(help.contains("--rsync"));
    }

    #[test]
    fn parses_duplicate_and_proxy_jump_clear() {
        let cli =
            Cli::try_parse_from(["sshnav", "host", "duplicate", "prod", "prod-copy"]).unwrap();
        let Some(Command::Host(HostCommand::Duplicate(args))) = cli.command else {
            panic!("expected duplicate command");
        };
        assert_eq!(args.source_alias, "prod");
        assert_eq!(args.new_alias, "prod-copy");

        let cli =
            Cli::try_parse_from(["sshnav", "host", "edit", "prod", "--no-proxy-jump"]).unwrap();
        let Some(Command::Host(HostCommand::Edit(args))) = cli.command else {
            panic!("expected edit command");
        };
        assert!(args.no_proxy_jump);
    }

    #[test]
    fn duplicate_copies_complete_host_and_independent_secret() {
        let dir = tempfile::tempdir().unwrap();
        let paths = test_paths(dir.path());
        let mut store = Store::open(&paths).unwrap();
        let mut source = Host::new("prod".into(), "prod.example".into());
        source.display_name = Some("Production".into());
        source.group = Some("work".into());
        source.tags = vec!["prod".into(), "db".into()];
        source.user = Some("ubuntu".into());
        source.port = Some(2222);
        source.private_key_source_path = Some("/keys/prod".into());
        source.template = Some("ssh".into());
        source.proxy_jump = Some("bastion".into());
        source.local_forwards = vec!["15432 localhost:5432".into()];
        source.remote_forwards = vec!["8080 localhost:80".into()];
        source.dynamic_forwards = vec!["1080".into()];
        source.options = vec!["Compression yes".into()];
        source.auto_reconnect = true;
        let mut inventory = Inventory::default();
        inventory.hosts.push(source);
        store.save_inventory(&inventory).unwrap();
        let blob = SecretBlob {
            salt: vec![1],
            nonce: vec![2],
            ciphertext: vec![3],
            source_path: Some("/keys/prod".into()),
        };
        store.put_secret("prod", SECRET_PRIVATE_KEY, &blob).unwrap();

        duplicate_host(&mut store, "prod", "prod-copy").unwrap();
        assert!(duplicate_host(&mut store, "prod", "prod-copy").is_err());
        assert!(duplicate_host(&mut store, "missing", "other-copy").is_err());
        let inventory = store.load_inventory().unwrap();
        let source = inventory.find_host("prod").unwrap();
        let mut duplicate = inventory.find_host("prod-copy").unwrap().clone();
        duplicate.alias = "prod".into();
        assert_eq!(&duplicate, source);

        remove_host(&mut store, "prod").unwrap();
        assert!(
            store
                .get_secret("prod-copy", SECRET_PRIVATE_KEY)
                .unwrap()
                .is_some()
        );
    }
}
