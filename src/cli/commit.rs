use std::path::{Path, PathBuf};

use crate::cli::build::{config_dir_of, load_config};
use crate::error::NythError;
use crate::cli::status::{DotfilesRepo, PendingChange, nyth_status};
use crate::sys::paths::{NythPaths, resolve_identity_and_paths};

/// Which pending changes `nyth commit` should actually write back to the repo
#[derive(Debug, Clone)]
pub enum CommitSelection {
    All,
    Modules(Vec<String>),
}

#[derive(Debug, Clone)]
pub struct CommitReport {
    /// Repo paths that got written, in the order they were applied
    pub applied: Vec<PathBuf>,
}

/// Which pending changes match `selection`. `Untracked` changes are never selected, regardless of the filter
pub fn select_changes_to_apply(
    pending: &[PendingChange],
    selection: &CommitSelection,
) -> Vec<PendingChange> {
    pending
        .iter()
        .filter(|change| match change {
            PendingChange::ModuleModified { module, .. } => match selection {
                CommitSelection::All => true,
                CommitSelection::Modules(names) => names.iter().any(|name| name == module),
            },
            PendingChange::Untracked { .. } => false,
        })
        .cloned()
        .collect()
}

/// Resolves the caller's identity-scoped paths for real, then commits.
/// Thin wrapper around `commit_into`, same split as `build`/`session`
pub fn commit(config_path: &Path) -> Result<CommitReport, NythError> {
    let (_, paths) = resolve_identity_and_paths()?;
    commit_into(config_path, &paths)
}

pub fn commit_into(config_path: &Path, paths: &NythPaths) -> Result<CommitReport, NythError> {
    let config = load_config(config_path)?;
    let config_dir = config_dir_of(config_path);
    let repo = DotfilesRepo::new(config_dir.to_path_buf(), config.modules);

    let pending = nyth_status(paths, &repo).map_err(NythError::Status)?;
    let selected = select_changes_to_apply(&pending, &CommitSelection::All);

    apply_commit(&selected, paths, &repo)
}

/// Writes each already-selected change back to its module's source in the local repo
pub fn apply_commit(
    selected: &[PendingChange],
    paths: &NythPaths,
    repo: &DotfilesRepo,
) -> Result<CommitReport, NythError> {
    let mut applied = Vec::with_capacity(selected.len());

    for change in selected {
        applied.push(apply_one_change(paths, repo, change)?);
    }

    Ok(CommitReport { applied })
}

fn apply_one_change(
    paths: &NythPaths,
    repo: &DotfilesRepo,
    change: &PendingChange,
) -> Result<PathBuf, NythError> {
    let PendingChange::ModuleModified {
        module: module_name,
        relative_path,
    } = change
    else {
        // select_changes_to_apply already filters these out
        return Err(NythError::CommitIoFailed {
            path: PathBuf::new(),
            message: "cannot commit an untracked path, no repo destination for it".to_string(),
        });
    };

    let (_, module) = repo
        .modules
        .iter()
        .find(|(name, _)| name == module_name)
        .expect("select_changes_to_apply only returns changes for modules that exist in repo");

    let remainder = relative_path
        .strip_prefix(module.target.as_path())
        .unwrap_or(Path::new(""));

    let source_in_upper = paths.upper.join(relative_path);
    let destination = if remainder.as_os_str().is_empty() {
        repo.root.join(&module.source)
    } else {
        repo.root.join(&module.source).join(remainder)
    };

    copy_one(&source_in_upper, &destination)?;
    Ok(destination)
}

// Delegates to the same symlink-preserving copy build.rs uses, just wraps
// the io::Error into this module's own error variant.
fn copy_one(source: &Path, destination: &Path) -> Result<(), NythError> {
    crate::fs_util::copy_file_preserving_symlinks(source, destination)
        .map_err(|e| commit_io_failed(destination, &e))
}

fn commit_io_failed(path: &Path, e: &std::io::Error) -> NythError {
    NythError::CommitIoFailed {
        path: path.to_path_buf(),
        message: e.to_string(),
    }
}
