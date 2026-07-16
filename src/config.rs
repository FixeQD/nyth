use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;

use crate::error::ConfigInvalidReason;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelativeHomePath(PathBuf);

#[derive(Debug)]
pub enum RelativeHomePathError {
    AbsolutePath(PathBuf),
    EscapesHome(PathBuf),
}

impl fmt::Display for RelativeHomePathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AbsolutePath(path) => write!(
                f,
                "target path {} must be relative to $HOME, not absolute",
                path.display()
            ),
            Self::EscapesHome(path) => write!(
                f,
                "target path {} contains '..' and would escape $HOME",
                path.display()
            ),
        }
    }
}

impl std::error::Error for RelativeHomePathError {}

impl RelativeHomePath {
    pub fn new(path: impl Into<PathBuf>) -> Result<Self, RelativeHomePathError> {
        let path = path.into();

        if path.is_absolute() {
            return Err(RelativeHomePathError::AbsolutePath(path));
        }

        if path.components().any(|c| c == Component::ParentDir) {
            return Err(RelativeHomePathError::EscapesHome(path));
        }

        Ok(Self(path))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// A shell command string, run after a module's files change
/// ts not parsed or split into argv here
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReloadCommand(pub String);

#[derive(Debug, Clone)]
pub struct Module {
    pub source: PathBuf,
    pub target: RelativeHomePath,
    pub on_change: Option<ReloadCommand>,
}

#[derive(Debug, Clone)]
pub struct NythConfig {
    pub env: BTreeMap<String, String>,
    /// (module name, module) in the order modules should layer into lowerdir
    /// BTreeMap on the raw side already gives us a sorted by name order
    pub modules: Vec<(String, Module)>,
}

// Mirrors nyth.toml
// Domain types (RelativeHomePath, ReloadCommand) only show up after TryFrom, not at the serde boundary
#[derive(Debug, Deserialize)]
struct RawConfig {
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    modules: BTreeMap<String, RawModule>,
}

#[derive(Debug, Deserialize)]
struct RawModule {
    source: PathBuf,
    target: PathBuf,
    on_change: Option<String>,
}

pub fn parse_nyth_toml(source: &str) -> Result<NythConfig, ConfigInvalidReason> {
    let raw: RawConfig =
        toml::from_str(source).map_err(|e| ConfigInvalidReason::TomlParseFailed {
            message: e.to_string(),
        })?;

    let mut modules = Vec::with_capacity(raw.modules.len());
    for (name, raw_module) in raw.modules {
        let target = RelativeHomePath::new(raw_module.target).map_err(|_| {
            ConfigInvalidReason::InvalidTargetPath {
                module: name.clone(),
            }
        })?;

        modules.push((
            name,
            Module {
                source: raw_module.source,
                target,
                on_change: raw_module.on_change.map(ReloadCommand),
            },
        ));
    }

    Ok(NythConfig {
        env: raw.env,
        modules,
    })
}
