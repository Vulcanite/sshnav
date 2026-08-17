use crate::generator;
use crate::paths::AppPaths;
use crate::secrets;
use crate::storage::Store;
use anyhow::Result;
use std::fs;
use std::process::Command;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Ok,
    Warn,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub status: Status,
    pub name: &'static str,
    pub message: String,
}

pub fn run(paths: &AppPaths, store: &Store) -> Vec<Check> {
    let mut checks = vec![
        check_exists("database", &paths.db),
        check_optional_exists("generated_config", &paths.generated_config),
        check_ssh(),
        check_include(paths),
        check_owner_only_permissions("database_permissions", &paths.db),
        check_optional_not_writable("ssh_config_permissions", &paths.ssh_config),
    ];
    checks.extend(check_private_keys(store));
    checks
}

pub fn print(checks: &[Check]) {
    for check in checks {
        let marker = match check.status {
            Status::Ok => "ok",
            Status::Warn => "warn",
        };
        println!("{marker:4} {:24} {}", check.name, check.message);
    }
}

fn check_exists(name: &'static str, path: &std::path::Path) -> Check {
    if Store::db_path_exists(path) {
        Check {
            status: Status::Ok,
            name,
            message: path.display().to_string(),
        }
    } else {
        Check {
            status: Status::Warn,
            name,
            message: format!("missing {}", path.display()),
        }
    }
}

fn check_optional_exists(name: &'static str, path: &std::path::Path) -> Check {
    if path.exists() {
        Check {
            status: Status::Ok,
            name,
            message: path.display().to_string(),
        }
    } else {
        Check {
            status: Status::Ok,
            name,
            message: format!("not present: {}", path.display()),
        }
    }
}

fn check_ssh() -> Check {
    match Command::new("ssh").arg("-V").output() {
        Ok(_) => Check {
            status: Status::Ok,
            name: "ssh_binary",
            message: "ssh is available".into(),
        },
        Err(err) => Check {
            status: Status::Warn,
            name: "ssh_binary",
            message: format!("ssh not available: {err}"),
        },
    }
}

fn check_include(paths: &AppPaths) -> Check {
    match fs::read_to_string(&paths.ssh_config) {
        Ok(contents) if generator::has_include(&contents) => Check {
            status: Status::Ok,
            name: "include_status",
            message: "optional sshnav include is installed".into(),
        },
        Ok(_) => Check {
            status: Status::Ok,
            name: "include_status",
            message: "optional sshnav include is not installed".into(),
        },
        Err(_) => Check {
            status: Status::Ok,
            name: "include_status",
            message: "optional ssh config not present".into(),
        },
    }
}

fn check_private_keys(store: &Store) -> Vec<Check> {
    let Ok(inventory) = store.load_inventory() else {
        return vec![Check {
            status: Status::Warn,
            name: "private_keys",
            message: "could not load hosts for key checks".into(),
        }];
    };

    let mut checks = Vec::new();
    for host in inventory.hosts {
        if let Some(path) = &host.private_key_source_path {
            let expanded = secrets::expand_tilde(std::path::Path::new(path));
            if expanded.exists() {
                checks.push(Check {
                    status: Status::Ok,
                    name: "key_source_metadata",
                    message: format!("{} source path exists: {}", host.alias, expanded.display()),
                });
            } else if host.has_private_key {
                checks.push(Check {
                    status: Status::Ok,
                    name: "key_source_metadata",
                    message: format!(
                        "{} source path missing; encrypted key is still stored",
                        host.alias
                    ),
                });
            } else {
                checks.push(Check {
                    status: Status::Warn,
                    name: "key_source_metadata",
                    message: format!(
                        "{} source path missing and no encrypted key is stored: {}",
                        host.alias,
                        expanded.display()
                    ),
                });
            }
        }
    }
    checks
}

#[cfg(unix)]
fn check_owner_only_permissions(name: &'static str, path: &std::path::Path) -> Check {
    use std::os::unix::fs::PermissionsExt;

    match fs::metadata(path) {
        Ok(metadata) => {
            let mode = metadata.permissions().mode() & 0o777;
            if mode == 0o600 {
                Check {
                    status: Status::Ok,
                    name,
                    message: format!("{} mode {:o}", path.display(), mode),
                }
            } else {
                Check {
                    status: Status::Warn,
                    name,
                    message: format!("{} should be mode 600, found {:o}", path.display(), mode),
                }
            }
        }
        Err(err) => Check {
            status: Status::Warn,
            name,
            message: format!("could not stat {}: {err}", path.display()),
        },
    }
}

#[cfg(not(unix))]
fn check_owner_only_permissions(name: &'static str, path: &std::path::Path) -> Check {
    Check {
        status: Status::Ok,
        name,
        message: format!("permission check skipped for {}", path.display()),
    }
}

#[cfg(unix)]
fn check_not_group_world_writable(name: &'static str, path: &std::path::Path) -> Check {
    use std::os::unix::fs::PermissionsExt;

    match fs::metadata(path) {
        Ok(metadata) => {
            let mode = metadata.permissions().mode() & 0o777;
            if mode & 0o022 == 0 {
                Check {
                    status: Status::Ok,
                    name,
                    message: format!("{} mode {:o}", path.display(), mode),
                }
            } else {
                Check {
                    status: Status::Warn,
                    name,
                    message: format!("{} is group/world writable ({:o})", path.display(), mode),
                }
            }
        }
        Err(err) => Check {
            status: Status::Warn,
            name,
            message: format!("could not stat {}: {err}", path.display()),
        },
    }
}

#[cfg(not(unix))]
fn check_not_group_world_writable(name: &'static str, path: &std::path::Path) -> Check {
    Check {
        status: Status::Ok,
        name,
        message: format!("permission check skipped for {}", path.display()),
    }
}

fn check_optional_not_writable(name: &'static str, path: &std::path::Path) -> Check {
    if path.exists() {
        check_not_group_world_writable(name, path)
    } else {
        Check {
            status: Status::Ok,
            name,
            message: format!("not present: {}", path.display()),
        }
    }
}

pub fn has_warnings(checks: &[Check]) -> bool {
    checks.iter().any(|check| check.status == Status::Warn)
}

pub fn run_and_print(paths: &AppPaths, store: &Store) -> Result<i32> {
    let checks = run(paths, store);
    print(&checks);
    Ok(if has_warnings(&checks) { 1 } else { 0 })
}
