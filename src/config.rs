use std::fmt;
use std::path::{Component, Path, PathBuf};

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
