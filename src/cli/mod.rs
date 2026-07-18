use std::process::ExitCode;

pub mod commit;
pub mod session;
pub mod status;

use commit::commit;
use session::{parse_session_args, run_session};
use status::{parse_repo_args, status};

/// Dispatches on `args[1]` (the subcommand). `args[0]` is the program name,
/// same convention as `std::env::args()`, so callers can pass that straight
/// through without stripping anything first.
pub fn run(args: &[String]) -> ExitCode {
    match args.get(1).map(String::as_str) {
        Some("session") => run_session_cmd(&args[2..]),
        Some("status") => run_status(&args[2..]),
        Some("commit") => run_commit(&args[2..]),
        Some(other) => {
            eprintln!("nyth: unknown command '{other}'");
            ExitCode::FAILURE
        }
        None => {
            eprintln!("usage: nyth <session|status|commit> [args]");
            ExitCode::FAILURE
        }
    }
}

fn run_session_cmd(args: &[String]) -> ExitCode {
    let session_args = match parse_session_args(args) {
        Ok(session_args) => session_args,
        Err(e) => {
            eprintln!("nyth session: {e}");
            return ExitCode::FAILURE;
        }
    };

    let error = run_session(
        &session_args.watched_paths,
        &session_args.env_overrides,
        &session_args.target_command,
    );
    eprintln!("nyth session failed: {error}");
    ExitCode::FAILURE
}

fn run_status(args: &[String]) -> ExitCode {
    let repo = match parse_repo_args(args) {
        Ok(repo_args) => repo_args.into_repo(),
        Err(e) => {
            eprintln!("nyth status: {e}");
            return ExitCode::FAILURE;
        }
    };

    match status(&repo) {
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

fn run_commit(args: &[String]) -> ExitCode {
    let repo = match parse_repo_args(args) {
        Ok(repo_args) => repo_args.into_repo(),
        Err(e) => {
            eprintln!("nyth commit: {e}");
            return ExitCode::FAILURE;
        }
    };

    match commit(&repo) {
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
