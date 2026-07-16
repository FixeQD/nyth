use std::fs;
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};

use crate::cli::build::{config_dir_of, load_config};
use crate::config::Module;
use crate::error::{NythError, StatusError};
use crate::sys::paths::{NythPaths, resolve_identity_and_paths};

/// A path that changed during a session: it exists in `upper` because overlayfs copied it up on write, or created it fresh
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpperEntry {
    pub relative_path: PathBuf,
}

/// One outstanding difference between `upper` and the dotfiles repo
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PendingChange {
    /// Changed at a path some module owns: a real edit to a managed config.
    ModuleModified {
        module: String,
        relative_path: PathBuf,
    },
    /// Changed at a path no module owns: something new, not managed by any module in nyth.toml
    Untracked { relative_path: PathBuf },
}

/// The dotfiles source of truth: `nyth.toml`'s modules, enough to tell which module owns a given $HOME-relative path
pub struct DotfilesRepo {
    pub root: PathBuf,
    pub modules: Vec<(String, Module)>,
}

impl DotfilesRepo {
    pub fn new(root: PathBuf, modules: Vec<(String, Module)>) -> Self {
        Self { root, modules }
    }

    /// Pure lookup: which module owns `entry`'s path
    pub fn compare(&self, entry: &UpperEntry) -> Option<PendingChange> {
        let owner = self.modules.iter().find(|(_, module)| {
            let target = module.target.as_path();
            entry.relative_path == target || entry.relative_path.starts_with(target)
        });

        Some(match owner {
            Some((name, _)) => PendingChange::ModuleModified {
                module: name.clone(),
                relative_path: entry.relative_path.clone(),
            },
            None => PendingChange::Untracked {
                relative_path: entry.relative_path.clone(),
            },
        })
    }
}

/// Resolves the caller's identity-scoped paths and config fr, then reports pending changes
pub fn status(config_path: &Path) -> Result<Vec<PendingChange>, NythError> {
    let (_, paths) = resolve_identity_and_paths()?;

    let config = load_config(config_path)?;
    let config_dir = config_dir_of(config_path);
    let repo = DotfilesRepo::new(config_dir.to_path_buf(), config.modules);

    nyth_status(&paths, &repo).map_err(NythError::Status)
}

/// Given what's already in `upper` and what the repo knows about, which changes are pending
pub fn diff_upper_against_repo(
    upper_entries: &[UpperEntry],
    repo: &DotfilesRepo,
) -> Vec<PendingChange> {
    upper_entries
        .iter()
        .filter_map(|entry| repo.compare(entry))
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
