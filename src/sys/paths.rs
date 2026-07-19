use std::path::PathBuf;

/// Persistent, per-target-user paths under `/run/nyth/<name>/`, a root-owned tmpfs
#[derive(Debug, Clone)]
pub struct NythPaths {
    pub root: PathBuf,
    pub lower: PathBuf,
    pub home_snapshot: PathBuf,
    pub upper: PathBuf,
    pub work: PathBuf,
}

impl NythPaths {
    /// `name` is the target username from `--for-user`, never the caller's own identity
    pub fn for_user(name: &str) -> Self {
        let root = PathBuf::from(format!("/run/nyth/{name}"));
        Self {
            lower: root.join("lower"),
            home_snapshot: root.join("home-snapshot"),
            upper: root.join("upper"),
            work: root.join("work"),
            root,
        }
    }
}
