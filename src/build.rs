use std::fs;
use std::path::Path;

use crate::config::{Module, NythConfig, parse_nyth_toml};
use crate::error::{ConfigInvalidReason, NythError};
use crate::sys::namespace::CallerIdentity;
use crate::sys::paths::NythPaths;

/// Resolves the caller's identity-scoped paths and regenerates `lower` from `config_path`
/// Thin wrapper around `build_into`, ts knows about `$HOME`/CallerIdentity
pub fn build(config_path: &Path) -> Result<NythPaths, NythError> {
    let identity = CallerIdentity::from_current_process().map_err(NythError::Namespace)?;
    let paths = NythPaths::for_identity(&identity);
    build_into(config_path, &paths.lower)?;
    Ok(paths)
}

/// Parses `config_path` and regenerates `lower` from its modules
/// Doesn't know about `$HOME` or identity at all, ts testable against any throwaway directory, no mounting, no `$HOME` involved
pub fn build_into(config_path: &Path, lower: &Path) -> Result<(), NythError> {
    let config = load_config(config_path)?;
    let config_dir = config_path.parent().unwrap_or_else(|| Path::new("."));

    reset_lower_dir(lower)?;

    for (name, module) in &config.modules {
        copy_module(config_dir, name, module, lower)?;
    }

    Ok(())
}

/// Reads and parses `config_path`, shared by `build` and `session` so there's one place that turns "file on disk" into `NythError`
pub fn load_config(config_path: &Path) -> Result<NythConfig, NythError> {
    let raw = fs::read_to_string(config_path).map_err(|e| NythError::ConfigInvalid {
        path: config_path.to_path_buf(),
        reason: ConfigInvalidReason::ReadFailed {
            message: e.to_string(),
        },
    })?;

    parse_nyth_toml(&raw).map_err(|reason| NythError::ConfigInvalid {
        path: config_path.to_path_buf(),
        reason,
    })
}

// lower is fully regenerated on every build, not merged with what was there: a module removed from nyth.toml must disappear from lower too.
fn reset_lower_dir(lower: &Path) -> Result<(), NythError> {
    if lower.exists() {
        fs::remove_dir_all(lower).map_err(|e| NythError::BuildIoFailed {
            path: lower.to_path_buf(),
            message: e.to_string(),
        })?;
    }
    fs::create_dir_all(lower).map_err(|e| NythError::BuildIoFailed {
        path: lower.to_path_buf(),
        message: e.to_string(),
    })
}

fn copy_module(
    config_dir: &Path,
    name: &str,
    module: &Module,
    lower: &Path,
) -> Result<(), NythError> {
    let source = config_dir.join(&module.source);
    let destination = lower.join(module.target.as_path());

    let metadata = fs::symlink_metadata(&source).map_err(|e| module_build_failed(name, &e))?;

    if metadata.is_dir() {
        copy_dir_recursive(&source, &destination).map_err(|e| module_build_failed(name, &e))
    } else {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent).map_err(|e| module_build_failed(name, &e))?;
        }
        fs::copy(&source, &destination)
            .map(|_| ())
            .map_err(|e| module_build_failed(name, &e))
    }
}

fn module_build_failed(module: &str, e: &std::io::Error) -> NythError {
    NythError::ModuleBuildFailed {
        module: module.to_string(),
        message: e.to_string(),
    }
}

// Symlinks inside a module's source are recreated as symlinks in lower, not silently dereferenced
fn copy_dir_recursive(source: &Path, destination: &Path) -> std::io::Result<()> {
    fs::create_dir_all(destination)?;

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let dest_path = destination.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir_recursive(&entry.path(), &dest_path)?;
        } else if file_type.is_symlink() {
            let link_target = fs::read_link(entry.path())?;
            std::os::unix::fs::symlink(&link_target, &dest_path)?;
        } else {
            fs::copy(entry.path(), &dest_path)?;
        }
    }

    Ok(())
}
