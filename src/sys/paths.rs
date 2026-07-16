use std::path::PathBuf;

use crate::sys::namespace::CallerIdentity;

/// Persistent, identity-scoped paths for nyth's own state. Separate from ScratchTmpfs (per-session):
/// `lower`/`upper`/`work` all have to survive across separate process invocations
#[derive(Debug, Clone)]
pub struct NythPaths {
    pub root: PathBuf,
    pub lower: PathBuf,
    pub upper: PathBuf,
    pub work: PathBuf,
}

impl NythPaths {
    pub fn for_identity(identity: &CallerIdentity) -> Self {
        let root = identity.home.join(".local/state/nyth");
        Self {
            lower: root.join("lower"),
            upper: root.join("upper"),
            work: root.join("work"),
            root,
        }
    }
}
