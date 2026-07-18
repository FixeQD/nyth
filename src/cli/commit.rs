use std::path::{Path, PathBuf};

use crate::cli::status::{DotfilesRepo, PendingChange, nyth_status};
use crate::config::RelativeHomePath;
use crate::error::{NotCommittableReason, NythError};
use crate::sys::paths::{NythPaths, resolve_identity_and_paths};

/// Which pending changes `nyth commit` should actually write back to the repo
#[derive(Debug, Clone)]
pub enum CommitSelection {
    All,
    WatchedPaths(Vec<RelativeHomePath>),
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
            PendingChange::RepoBacked { relative_path } => match selection {
                CommitSelection::All => true,
                CommitSelection::WatchedPaths(paths) => paths.iter().any(|watched| {
                    let target = watched.as_path();
                    relative_path == target || relative_path.starts_with(target)
                }),
            },
            PendingChange::Generated { .. } | PendingChange::Untracked { .. } => false,
        })
        .cloned()
        .collect()
}

/// Resolves the caller's identity-scoped paths for real, then commits.
/// Thin wrapper around `commit_into`, same split as `session`
pub fn commit(repo: &DotfilesRepo) -> Result<CommitReport, NythError> {
    let (_, paths) = resolve_identity_and_paths()?;
    commit_into(repo, &paths)
}

pub fn commit_into(repo: &DotfilesRepo, paths: &NythPaths) -> Result<CommitReport, NythError> {
    let pending = nyth_status(paths, repo).map_err(NythError::Status)?;
    let selected = select_changes_to_apply(&pending, &CommitSelection::All);

    apply_commit(&selected, paths, repo)
}

/// Writes each already-selected change back to the repo, at the same $HOME-relative path it changed at: the repo mirrors $HOME under `repo.root`
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
    let relative_path = match change {
        PendingChange::RepoBacked { relative_path } => relative_path,
        PendingChange::Generated { relative_path } => {
            return Err(NythError::NotCommittable {
                path: relative_path.clone(),
                reason: NotCommittableReason::Generated,
            });
        }
        PendingChange::Untracked { relative_path } => {
            return Err(NythError::NotCommittable {
                path: relative_path.clone(),
                reason: NotCommittableReason::Untracked,
            });
        }
    };

    let source_in_upper = paths.upper.join(relative_path);
    let destination = repo.root.join(relative_path);

    copy_one(&source_in_upper, &destination)?;
    Ok(destination)
}

// Same symlink-preserving copy fs_util provides everywhere else in the crate,
// just wraps the io::Error into this module's own error variant.
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
