use std::fmt;
use std::path::PathBuf;

#[derive(Debug)]
pub enum NamespaceError {
    UnshareFailed { errno: i32 },
    UserNamespacesDisabled,
    SetgroupsWriteFailed { errno: i32 },
    UidMapWriteFailed { errno: i32 },
    MountPropagationFailed { errno: i32 },
    HomeLookupFailed { uid: u32, errno: i32 },
}

impl fmt::Display for NamespaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnshareFailed { errno } => {
                write!(f, "failed to unshare user/mount namespaces (errno {errno})")
            }
            Self::UserNamespacesDisabled => write!(
                f,
                "user namespaces are disabled (check security.allowUserNamespaces or a hardened kernel profile)"
            ),
            Self::SetgroupsWriteFailed { errno } => {
                write!(f, "failed to write /proc/self/setgroups (errno {errno})")
            }
            Self::UidMapWriteFailed { errno } => {
                write!(f, "failed to write uid/gid map (errno {errno})")
            }
            Self::MountPropagationFailed { errno } => {
                write!(f, "failed to make mount tree private (errno {errno})")
            }
            Self::HomeLookupFailed { uid, errno: 0 } => {
                write!(f, "no passwd entry found for uid {uid}")
            }
            Self::HomeLookupFailed { uid, errno } => {
                write!(
                    f,
                    "failed to look up home directory for uid {uid} (errno {errno})"
                )
            }
        }
    }
}

impl std::error::Error for NamespaceError {}

#[derive(Debug)]
pub enum OverlayError {
    ScratchDirCreateFailed { errno: i32 },
    ScratchTmpfsMountFailed { errno: i32 },
    ScratchSubdirFailed { path: PathBuf, errno: i32 },
    HomeSnapshotFailed { errno: i32 },
    WatchedPathUnresolved { path: PathBuf, errno: i32 },
    OverlayApiUnsupported { errno: i32 },
    MountFailed { target: PathBuf, errno: i32 },
}

impl fmt::Display for OverlayError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScratchDirCreateFailed { errno } => {
                write!(
                    f,
                    "failed to create scratch tmpfs directory (errno {errno})"
                )
            }
            Self::ScratchTmpfsMountFailed { errno } => {
                write!(f, "failed to mount scratch tmpfs (errno {errno})")
            }
            Self::ScratchSubdirFailed { path, errno } => {
                write!(f, "failed to create {} (errno {errno})", path.display())
            }
            Self::HomeSnapshotFailed { errno } => {
                write!(
                    f,
                    "failed to create read-only home snapshot (errno {errno})"
                )
            }
            Self::WatchedPathUnresolved { path, errno } => write!(
                f,
                "failed to resolve watched path {} to its real /nix/store target (errno {errno})",
                path.display()
            ),
            Self::OverlayApiUnsupported { errno } => write!(
                f,
                "kernel too old or overlay filesystem module not loaded (errno {errno})"
            ),
            Self::MountFailed { target, errno } => {
                write!(
                    f,
                    "failed to mount overlay at {} (errno {errno})",
                    target.display()
                )
            }
        }
    }
}

impl std::error::Error for OverlayError {}

#[derive(Debug)]
pub enum ConfigInvalidReason {
    ReadFailed { message: String },
    InvalidTargetPath { module: String },
    TomlParseFailed { message: String },
}

impl fmt::Display for ConfigInvalidReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReadFailed { message } => write!(f, "failed to read config file: {message}"),
            Self::InvalidTargetPath { module } => {
                write!(f, "module '{module}' has an invalid target path")
            }
            Self::TomlParseFailed { message } => write!(f, "failed to parse TOML: {message}"),
        }
    }
}

impl std::error::Error for ConfigInvalidReason {}

#[derive(Debug)]
pub enum NythError {
    Namespace(NamespaceError),
    Overlay(OverlayError),
    Status(StatusError),
    ConfigInvalid {
        path: PathBuf,
        reason: ConfigInvalidReason,
    },
    ModuleTargetEscapesHome {
        module: String,
        target: PathBuf,
    },
    ModuleBuildFailed {
        module: String,
        message: String,
    },
    BuildIoFailed {
        path: PathBuf,
        message: String,
    },
    SessionIoFailed {
        path: PathBuf,
        message: String,
    },
    CommitIoFailed {
        path: PathBuf,
        message: String,
    },
    ExecFailed {
        program: String,
        message: String,
    },
    NotBuilt {
        expected_lower: PathBuf,
    },
    NoTargetCommand,
}

impl fmt::Display for NythError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Namespace(e) => write!(f, "{e}"),
            Self::Overlay(e) => write!(f, "{e}"),
            Self::Status(e) => write!(f, "{e}"),
            Self::ConfigInvalid { path, reason } => {
                write!(f, "invalid config at {}: {reason}", path.display())
            }
            Self::ModuleTargetEscapesHome { module, target } => write!(
                f,
                "module '{module}' target {} escapes $HOME",
                target.display()
            ),
            Self::ModuleBuildFailed { module, message } => {
                write!(f, "failed to build module '{module}': {message}")
            }
            Self::BuildIoFailed { path, message } => {
                write!(f, "build failed at {}: {message}", path.display())
            }
            Self::SessionIoFailed { path, message } => {
                write!(f, "session setup failed at {}: {message}", path.display())
            }
            Self::CommitIoFailed { path, message } => {
                write!(f, "commit failed at {}: {message}", path.display())
            }
            Self::ExecFailed { program, message } => {
                write!(f, "failed to exec '{program}': {message}")
            }
            Self::NotBuilt { expected_lower } => write!(
                f,
                "session not built yet, expected lower dir at {}",
                expected_lower.display()
            ),
            Self::NoTargetCommand => write!(f, "no command given to run inside the session"),
        }
    }
}

impl std::error::Error for NythError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Namespace(e) => Some(e),
            Self::Overlay(e) => Some(e),
            Self::Status(e) => Some(e),
            Self::ConfigInvalid { reason, .. } => Some(reason),
            _ => None,
        }
    }
}

#[derive(Debug)]
pub enum StatusError {
    ScanFailed { path: PathBuf, message: String },
}

impl fmt::Display for StatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ScanFailed { path, message } => {
                write!(f, "failed to scan {}: {message}", path.display())
            }
        }
    }
}

impl std::error::Error for StatusError {}
