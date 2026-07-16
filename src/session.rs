use std::path::PathBuf;

/// The overlay session's lifecycle: built -> mounted -> unmounted
/// A sum type instead of one struct with `Option<PathBuf>` per stage
#[derive(Debug, Clone)]
pub enum SessionState {
    Built {
        lower_dir: PathBuf,
    },
    Mounted {
        lower_dir: PathBuf,
        upper_dir: PathBuf,
        work_dir: PathBuf,
        home_target: PathBuf,
    },
    Unmounted {
        upper_dir: PathBuf,
    },
}
