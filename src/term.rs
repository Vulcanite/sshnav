const LIGHT_RED: &str = "\x1b[91m";
const GREEN: &str = "\x1b[92m";
const CYAN: &str = "\x1b[96m";
const YELLOW: &str = "\x1b[93m";
const RESET: &str = "\x1b[0m";

pub fn error(message: impl std::fmt::Display) {
    eprintln!("{LIGHT_RED}{message}{RESET}");
}

pub fn success(message: impl std::fmt::Display) {
    println!("{GREEN}{message}{RESET}");
}

pub fn info(message: impl std::fmt::Display) {
    println!("{CYAN}{message}{RESET}");
}

pub fn warn(message: impl std::fmt::Display) {
    eprintln!("{YELLOW}{message}{RESET}");
}
