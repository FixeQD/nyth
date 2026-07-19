use std::fmt;

use crate::config::{RelativeHomePath, RelativeHomePathError};
use crate::error::{NythError, OverlayError};
use crate::sys::identity::{TargetIdentity, require_real_root};
use crate::sys::overlay::{
    OverlayState, current_overlay_state, mount_home_snapshot, mount_overlay,
    provision_persistent_tmpfs, resolve_watched_paths, set_ownership,
};
use crate::sys::paths::NythPaths;

/// argv-parsed inputs to `nyth mount`
#[derive(Debug, Clone, Default)]
pub struct MountArgs {
    pub for_user: String,
    pub watched_paths: Vec<RelativeHomePath>,
}

#[derive(Debug)]
pub enum MountArgsError {
    MissingFlagValue {
        flag: &'static str,
    },
    MissingForUser,
    InvalidWatchedPath {
        raw: String,
        source: RelativeHomePathError,
    },
    UnexpectedArgument {
        raw: String,
    },
}

impl fmt::Display for MountArgsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFlagValue { flag } => write!(f, "{flag} requires a value"),
            Self::MissingForUser => write!(f, "--for-user <name> is required"),
            Self::InvalidWatchedPath { raw, source } => {
                write!(f, "invalid --watched-path '{raw}': {source}")
            }
            Self::UnexpectedArgument { raw } => write!(
                f,
                "unexpected argument '{raw}', expected --for-user or --watched-path"
            ),
        }
    }
}

impl std::error::Error for MountArgsError {}

/// Parses `nyth mount --for-user <name> [--watched-path <rel>]...`
pub fn parse_mount_args(args: &[String]) -> Result<MountArgs, MountArgsError> {
    let mut for_user = None;
    let mut watched_paths = Vec::new();
    let mut remaining = args.iter();

    while let Some(arg) = remaining.next() {
        match arg.as_str() {
            "--for-user" => {
                let raw = remaining
                    .next()
                    .ok_or(MountArgsError::MissingFlagValue { flag: "--for-user" })?;
                for_user = Some(raw.clone());
            }
            "--watched-path" => {
                let raw = remaining.next().ok_or(MountArgsError::MissingFlagValue {
                    flag: "--watched-path",
                })?;
                let path = RelativeHomePath::new(raw.as_str()).map_err(|source| {
                    MountArgsError::InvalidWatchedPath {
                        raw: raw.clone(),
                        source,
                    }
                })?;
                watched_paths.push(path);
            }
            other => {
                return Err(MountArgsError::UnexpectedArgument {
                    raw: other.to_string(),
                });
            }
        }
    }

    Ok(MountArgs {
        for_user: for_user.ok_or(MountArgsError::MissingForUser)?,
        watched_paths,
    })
}

/// `nyth mount`: checks `geteuid() == 0`, resolves the target's identity, provisions `/run/nyth/<name>/`, snapshots the target's real $HOME read-only, resolves watched-paths into `lower/`, and mounts the overlay over the target's $HOME
pub fn run_mount(args: &MountArgs) -> Result<(), NythError> {
    require_real_root().map_err(NythError::Identity)?;
    let identity = TargetIdentity::from_username(&args.for_user).map_err(NythError::Identity)?;

    let already_mounted = current_overlay_state(&identity.home).map_err(NythError::Overlay)?;
    if already_mounted == OverlayState::Mounted {
        return Err(NythError::Overlay(OverlayError::AlreadyMounted {
            user: args.for_user.clone(),
        }));
    }

    let paths = NythPaths::for_user(&args.for_user);

    provision_persistent_tmpfs(&paths).map_err(NythError::Overlay)?;
    mount_home_snapshot(&identity.home, &paths).map_err(NythError::Overlay)?;
    resolve_watched_paths(&paths, &args.watched_paths).map_err(NythError::Overlay)?;

    // upper/work are created by root; the target user's own processes running inside the overlay need to be able to write to them
    set_ownership(&paths.upper, identity.uid, identity.gid).map_err(NythError::Overlay)?;
    set_ownership(&paths.work, identity.uid, identity.gid).map_err(NythError::Overlay)?;

    mount_overlay(&paths, &identity.home).map_err(NythError::Overlay)
}
