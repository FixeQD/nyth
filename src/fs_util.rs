use std::fs;
use std::path::Path;

/// Copies a single file to `destination`, creating its parent dirs first.
/// Symlinks are recreated as symlinks, not dereferenced: `fs::copy()` on a
/// symlink would follow it and copy the target's content under the link's
/// name, silently changing what kind of file ends up at the destination.
///
/// Shared by `build` (repo -> lower) and `commit` (upper -> repo), same
/// direction-agnostic operation either way: copy one file, preserve its
/// symlink-ness.
pub fn copy_file_preserving_symlinks(source: &Path, destination: &Path) -> std::io::Result<()> {
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }

    let metadata = fs::symlink_metadata(source)?;

    if metadata.is_symlink() {
        let link_target = fs::read_link(source)?;
        let _ = fs::remove_file(destination);
        std::os::unix::fs::symlink(&link_target, destination)
    } else {
        fs::copy(source, destination).map(|_| ())
    }
}
