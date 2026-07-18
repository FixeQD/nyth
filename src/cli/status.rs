use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

use crate::config::RelativeHomePath;
use crate::error::{NythError, StatusError};
use crate::sys::paths::{NythPaths, resolve_identity_and_paths};

/// A path that changed during a session: it exists in `upper` because overlayfs copied it up on write, or created it fresh
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpperEntry {
    pub relative_path: PathBuf,
}

/// One outstanding difference between `upper` and what Home Manager knows about that path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingChange {
    /// Watched by Home Manager, and backed by a real source file in the dotfiles repo
    /// (`home.file.<name>.source` pointing at a path the user wrote not rendered one)
    RepoBacked { relative_path: PathBuf },
    /// Watched by Home Manager, but rendered by a `programs.*` module from Nix options
    /// (`home.file.<name>.text` was set)
    Generated { relative_path: PathBuf },
    /// Changed at a path nobody watches: something new, not managed by Home Manager at all
    Untracked { relative_path: PathBuf },
}

/// The dotfiles source of truth: `root` is a repo that mirrors $HOME 1:1 for the repo-backed subset
pub struct DotfilesRepo {
    pub root: PathBuf,
    pub repo_backed_paths: Vec<RelativeHomePath>,
    pub generated_paths: Vec<RelativeHomePath>,
}

impl DotfilesRepo {
    pub fn new(
        root: PathBuf,
        repo_backed_paths: Vec<RelativeHomePath>,
        generated_paths: Vec<RelativeHomePath>,
    ) -> Self {
        Self {
            root,
            repo_backed_paths,
            generated_paths,
        }
    }

    /// Pure lookup: which of the three states does `entry`'s path fall into
    pub fn compare(&self, entry: &UpperEntry) -> PendingChange {
        if matches_watched_path(&self.repo_backed_paths, &entry.relative_path) {
            PendingChange::RepoBacked {
                relative_path: entry.relative_path.clone(),
            }
        } else if matches_watched_path(&self.generated_paths, &entry.relative_path) {
            PendingChange::Generated {
                relative_path: entry.relative_path.clone(),
            }
        } else {
            PendingChange::Untracked {
                relative_path: entry.relative_path.clone(),
            }
        }
    }
}

/// `path` matches a watched-path if it's literally that path, or nested under it
fn matches_watched_path(watched_paths: &[RelativeHomePath], path: &Path) -> bool {
    watched_paths
        .iter()
        .any(|watched| path == watched.as_path() || path.starts_with(watched.as_path()))
}

/// Resolves the caller's identity-scoped paths, then reports pending changes against `repo`
pub fn status(repo: &DotfilesRepo) -> Result<Vec<PendingChange>, NythError> {
    let (_, paths) = resolve_identity_and_paths()?;
    nyth_status(&paths, repo).map_err(NythError::Status)
}

/// Given what's already in `upper` and what the repo knows about, which changes are pending
pub fn diff_upper_against_repo(
    upper_entries: &[UpperEntry],
    repo: &DotfilesRepo,
) -> Vec<PendingChange> {
    upper_entries
        .iter()
        .map(|entry| repo.compare(entry))
        .collect()
}

/// Effect: The only I/O in the whole `status` path. Walks `upper` once, hands the result to the pure decision above
pub fn nyth_status(
    paths: &NythPaths,
    repo: &DotfilesRepo,
) -> Result<Vec<PendingChange>, StatusError> {
    let upper_entries = scan_upper_dir(&paths.upper)?;
    Ok(diff_upper_against_repo(&upper_entries, repo))
}

fn scan_upper_dir(upper_dir: &Path) -> Result<Vec<UpperEntry>, StatusError> {
    let mut entries = Vec::new();
    walk(upper_dir, upper_dir, &mut entries)?;
    Ok(entries)
}

fn walk(root: &Path, dir: &Path, out: &mut Vec<UpperEntry>) -> Result<(), StatusError> {
    let read_dir = fs::read_dir(dir).map_err(|e| StatusError::ScanFailed {
        path: dir.to_path_buf(),
        message: e.to_string(),
    })?;

    for entry in read_dir {
        let entry = entry.map_err(|e| StatusError::ScanFailed {
            path: dir.to_path_buf(),
            message: e.to_string(),
        })?;
        let file_type = entry.file_type().map_err(|e| StatusError::ScanFailed {
            path: entry.path(),
            message: e.to_string(),
        })?;

        if file_type.is_char_device() {
            continue;
        }

        if file_type.is_dir() {
            walk(root, &entry.path(), out)?;
            continue;
        }

        let relative_path = entry
            .path()
            .strip_prefix(root)
            .expect("entry is always under root, we just walked it from there")
            .to_path_buf();
        out.push(UpperEntry { relative_path });
    }

    Ok(())
}
