use std::path::{Path, PathBuf};
use std::process::ExitCode;

pub mod commit;
pub mod generated_diff;
pub mod session;
pub mod status;

use commit::commit;
use generated_diff::{read_generated_change, render_generated_change};
use session::{parse_session_args, run_session};
use status::{PendingChange, parse_repo_args, status};

use crate::sys::paths::resolve_identity_and_paths;

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
        session_args.target_command.as_deref(),
    );
    eprintln!("nyth session failed: {error}");
    ExitCode::FAILURE
}

/// Non-overlaid $HOME (before) and this identity's upper dir (after)
struct DiffRoots {
    home: PathBuf,
    upper: PathBuf,
}

fn run_status(args: &[String]) -> ExitCode {
    let repo = match parse_repo_args(args) {
        Ok(repo_args) => repo_args.into_repo(),
        Err(e) => {
            eprintln!("nyth status: {e}");
            return ExitCode::FAILURE;
        }
    };

    let changes = match status(&repo) {
        Ok(changes) => changes,
        Err(e) => {
            eprintln!("nyth status failed: {e}");
            return ExitCode::FAILURE;
        }
    };

    if changes.is_empty() {
        println!("nothing to commit");
        return ExitCode::SUCCESS;
    }

    // Only Generated needs identity.home to diff against
    let diff_roots = resolve_identity_and_paths()
        .ok()
        .map(|(identity, paths)| DiffRoots {
            home: identity.home,
            upper: paths.upper,
        });

    for change in &changes {
        match change {
            PendingChange::Generated { relative_path } => {
                print_generated_change(diff_roots.as_ref(), relative_path);
            }
            other => println!("{other:?}"),
        }
    }

    ExitCode::SUCCESS
}

fn print_generated_change(diff_roots: Option<&DiffRoots>, relative_path: &Path) {
    let Some(roots) = diff_roots else {
        println!(
            "Generated {{ relative_path: {relative_path:?} }} (couldn't resolve identity, showing raw path only)"
        );
        return;
    };

    match read_generated_change(&roots.home, &roots.upper, relative_path) {
        Ok(change) => print!("{}", render_generated_change(&change)),
        Err(e) => println!("Generated {{ relative_path: {relative_path:?} }} ({e})"),
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
