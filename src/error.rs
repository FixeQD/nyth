use std::path::PathBuf;

#[derive(Debug)]
pub enum NamespaceError {
    UnshareFailed { errno: i32 },
    UserNamespacesDisabled,
    UidMapWriteFailed { errno: i32 },
    MountPropagationFailed { errno: i32 },
    ScratchTmpfsFailed { errno: i32 },
    HomeLookupFailed { uid: u32, errno: i32 },
}

#[derive(Debug)]
pub enum OverlayError {
    HomeSnapshotFailed { errno: i32 },
    MountFailed { target: PathBuf, errno: i32 },
}

#[derive(Debug)]
pub enum ConfigInvalidReason {
    MissingModulesTable,
    InvalidTargetPath { module: String },
    TomlParseFailed { message: String },
}

#[derive(Debug)]
pub enum NythError {
    Namespace(NamespaceError),
    Overlay(OverlayError),
    ConfigInvalid {
        path: PathBuf,
        reason: ConfigInvalidReason,
    },
    ModuleTargetEscapesHome {
        module: String,
        target: PathBuf,
    },
    NotBuilt {
        expected_lower: PathBuf,
    },
    NoTargetCommand,
}
