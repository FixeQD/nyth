use std::path::Path;
use std::process::ExitCode;

pub mod build;
pub mod commit;
pub mod session;
pub mod status;

use build::build;
use commit::commit;
use session::run_session;
use status::status;

/// Dispatches on `args[1]` (the subcommand). `args[0]` is the program name,
/// same convention as `std::env::args()`, so callers can pass that straight
/// through without stripping anything first.
pub fn run(args: &[String]) -> ExitCode {
    match args.get(1).map(String::as_str) {
        Some("build") => run_build(args.get(2).map(String::as_str)),
        Some("session") => run_session_cmd(&args[2..]),
        Some("status") => run_status(args.get(2).map(String::as_str)),
        Some("commit") => run_commit(args.get(2).map(String::as_str)),
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

fn run_session_cmd(target_command: &[String]) -> ExitCode {
    let config_path = Path::new("nyth.toml");
    let error = run_session(config_path, target_command);
    eprintln!("nyth session failed: {error}");
    ExitCode::FAILURE
}

fn run_status(config_arg: Option<&str>) -> ExitCode {
    let config_path = Path::new(config_arg.unwrap_or("nyth.toml"));

    match status(config_path) {
        Ok(changes) if changes.is_empty() => {
            println!("nothing to commit");
            ExitCode::SUCCESS
        }
        Ok(changes) => {
            for change in changes {
                println!("{change:?}");
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("nyth status failed: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run_commit(config_arg: Option<&str>) -> ExitCode {
    let config_path = Path::new(config_arg.unwrap_or("nyth.toml"));

    match commit(config_path) {
        Ok(report) if report.applied.is_empty() => {
            println!("nothing to commit");
            ExitCode::SUCCESS
        }
        Ok(report) => {
            for path in report.applied {
                println!("committed {}", path.display());
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("nyth commit failed: {e}");
            ExitCode::FAILURE
        }
    }
}
