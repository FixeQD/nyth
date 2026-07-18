use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub mod commit;
pub mod session;
pub mod status;

use commit::commit;
use session::{parse_session_args, run_session};
use status::{DotfilesRepo, status};

/// Dispatches on `args[1]` (the subcommand). `args[0]` is the program name,
/// same convention as `std::env::args()`, so callers can pass that straight
/// through without stripping anything first.
pub fn run(args: &[String]) -> ExitCode {
    match args.get(1).map(String::as_str) {
        Some("session") => run_session_cmd(&args[2..]),
        Some("status") => run_status(args.get(2).map(String::as_str)),
        Some("commit") => run_commit(args.get(2).map(String::as_str)),
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

fn run_status(repo_root_arg: Option<&str>) -> ExitCode {
    let repo = dotfiles_repo(repo_root_arg);

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

fn run_commit(repo_root_arg: Option<&str>) -> ExitCode {
    let repo = dotfiles_repo(repo_root_arg);

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

fn dotfiles_repo(repo_root_arg: Option<&str>) -> DotfilesRepo {
    let root: PathBuf = Path::new(repo_root_arg.unwrap_or(".")).to_path_buf();
    DotfilesRepo::new(root, Vec::new())
}
