use std::os::unix::process::CommandExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use crate::build::load_config;
use crate::error::NythError;
use crate::sys::namespace::{CallerIdentity, enter_isolated_session};
use crate::sys::overlay::{mount_home_snapshot, mount_overlay, provision_scratch_tmpfs};
use crate::sys::paths::NythPaths;

/// The overlay session's lifecycle: built -> mounted -> unmounted
/// A sum type instead of one struct with `Option<PathBuf>` per stage
#[derive(Debug, Clone)]
pub enum SessionState {
    Built {
        lower_dir: PathBuf,
    },
    Mounted {
        lower_dir: PathBuf,
        upper_dir: PathBuf,
        work_dir: PathBuf,
        home_target: PathBuf,
    },
    Unmounted {
        upper_dir: PathBuf,
    },
}

/// Resolves the caller's identity/paths fr and runs the session
/// Thin wrapper around `run_session_with`, same split as `build`/`build_into`
pub fn run_session(config_path: &Path, target_command: &[String]) -> NythError {
    if target_command.is_empty() {
        return NythError::NoTargetCommand;
    }

    let identity = match CallerIdentity::from_current_process() {
        Ok(identity) => identity,
        Err(e) => return NythError::Namespace(e),
    };
    let paths = NythPaths::for_identity(&identity);

    run_session_with(config_path, target_command, &identity, &paths)
}

/// Same as `run_session`, but takes `identity`/`paths` instead of computing them from the real process
pub fn run_session_with(
    config_path: &Path,
    target_command: &[String],
    identity: &CallerIdentity,
    paths: &NythPaths,
) -> NythError {
    let config = match load_config(config_path) {
        Ok(config) => config,
        Err(e) => return e,
    };

    if !paths.lower.exists() {
        return NythError::NotBuilt {
            expected_lower: paths.lower.clone(),
        };
    }

    if let Err(e) = enter_isolated_session(identity.uid, identity.gid) {
        return NythError::Namespace(e);
    }

    let scratch = match provision_scratch_tmpfs(identity) {
        Ok(scratch) => scratch,
        Err(e) => return NythError::Overlay(e),
    };

    if let Err(e) = mount_home_snapshot(&identity.home, &scratch) {
        return NythError::Overlay(e);
    }

    if let Err(e) = ensure_dir(&paths.upper) {
        return e;
    }
    if let Err(e) = ensure_dir(&paths.work) {
        return e;
    }

    if let Err(e) = mount_overlay(
        &paths.lower,
        &paths.upper,
        &paths.work,
        &scratch,
        &identity.home,
    ) {
        return NythError::Overlay(e);
    }

    exec_target(target_command, &config.env)
}

// upper/work are persistent, not part of the ephemeral scratch tmpfs, so unlike scratch's own subdirs they might not exist yet the very first time a session runs for this identity
fn ensure_dir(path: &Path) -> Result<(), NythError> {
    std::fs::create_dir_all(path).map_err(|e| NythError::SessionIoFailed {
        path: path.to_path_buf(),
        message: e.to_string(),
    })
}

// Only returns on failure: on success execvp() replaces this process image and this function stops existing
fn exec_target(command: &[String], env: &std::collections::BTreeMap<String, String>) -> NythError {
    let Some((program, args)) = command.split_first() else {
        return NythError::NoTargetCommand;
    };

    let error = Command::new(program).args(args).envs(env).exec();
    NythError::ExecFailed {
        program: program.clone(),
        message: error.to_string(),
    }
}
