use std::env;
use std::path::Path;
use std::process::ExitCode;

use nyth::build::build;
use nyth::session::run_session;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("build") => run_build(args.get(2).map(String::as_str)),
        Some("session") => run_session_cmd(&args[2..]),
        Some(other) => {
            eprintln!("nyth: unknown command '{other}'");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: nyth <build|session|status|commit> [args]");
            ExitCode::FAILURE
        }
    }
}

fn run_session_cmd(target_command: &[String]) -> ExitCode {
    let config_path = Path::new("nyth.toml");
    let error = run_session(config_path, target_command);
    eprintln!("nyth session failed: {error}");
    ExitCode::FAILURE
}

fn run_build(config_arg: Option<&str>) -> ExitCode {
    let config_path = Path::new(config_arg.unwrap_or("nyth.toml"));

    match build(config_path) {
        Ok(paths) => {
            println!("built lowerdir at {}", paths.lower.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("nyth build failed: {e}");
            ExitCode::FAILURE
        }
    }
}
