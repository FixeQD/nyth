use std::path::PathBuf;

use crate::error::NythError;
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

/// Resolves the real caller's identity and identity-scoped paths together.
/// Shared by session/status/commit, one place instead of three copies
/// of the same two lines.
pub fn resolve_identity_and_paths() -> Result<(CallerIdentity, NythPaths), NythError> {
    let identity = CallerIdentity::from_current_process().map_err(NythError::Namespace)?;
    let paths = NythPaths::for_identity(&identity);
    Ok((identity, paths))
}
