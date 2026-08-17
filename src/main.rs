mod add_form;
mod cli;
mod diagnostics;
mod doctor;
mod generator;
mod inventory;
mod paths;
mod picker;
mod runner;
mod secrets;
mod ssh_config;
mod storage;
mod term;

fn main() {
    match cli::run() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            term::error(format!("Error: {err:#}"));
            std::process::exit(1);
        }
    }
}
