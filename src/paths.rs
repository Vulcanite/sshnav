use anyhow::{Context, Result};
use directories::BaseDirs;
use std::env;
use std::path::PathBuf;

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub db: PathBuf,
    pub secret_key: PathBuf,
    pub runtime_dir: PathBuf,
    pub generated_config: PathBuf,
    pub ssh_config: PathBuf,
}

impl AppPaths {
    pub fn discover() -> Result<Self> {
        let home = env::var_os("SSHNAV_HOME")
            .map(PathBuf::from)
            .or_else(|| env::var_os("HOME").map(PathBuf::from))
            .or_else(|| BaseDirs::new().map(|dirs| dirs.home_dir().to_path_buf()))
            .context("could not determine home directory")?;

        let data_dir = env::var_os("SSHNAV_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local").join("share").join("sshnav"));
        let runtime_dir = env::var_os("SSHNAV_RUNTIME_DIR")
            .map(PathBuf::from)
            .or_else(|| env::var_os("XDG_RUNTIME_DIR").map(|dir| PathBuf::from(dir).join("sshnav")))
            .unwrap_or_else(|| data_dir.join("runtime"));
        let ssh_dir = home.join(".ssh");

        Ok(Self {
            db: data_dir.join("sshnav.db"),
            secret_key: data_dir.join("secret.key"),
            runtime_dir,
            generated_config: ssh_dir.join("sshnav.generated"),
            ssh_config: ssh_dir.join("config"),
        })
    }
}
