mod support;

use std::ffi::CString;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::Path;

use nyth::config::RelativeHomePath;
use nyth::error::OverlayError;
use nyth::sys::overlay::{
    mount_overlay, provision_persistent_tmpfs, resolve_watched_paths, unmount_persistent_tmpfs,
};
use nyth::sys::paths::NythPaths;

/// EPERM/EACCES on the very first root-only step (creating/mounting `/run/nyth/<name>`) means this process isn't running as real root - expected outside a CI container running as root
fn is_permission_denied(errno: i32) -> bool {
    errno == libc::EPERM || errno == libc::EACCES
}

fn umount_path(path: &Path) {
    let c = CString::new(path.as_os_str().as_bytes()).expect("path has no interior NUL");
    unsafe {
        libc::umount2(c.as_ptr(), 0);
    }
}

#[test]
fn mount_overlay_merges_lower_and_allows_writes() {
    support::run_in_fork(run_in_child);
}

fn run_in_child() -> i32 {
    let name = format!("nyth-test-overlay-{}", std::process::id());
    let paths = NythPaths::for_user(&name);

    if let Err(e) = provision_persistent_tmpfs(&paths) {
        if let OverlayError::PersistentTmpfsFailed { errno } = e {
            if is_permission_denied(errno) {
                return 0;
            }
        }
        eprintln!("provision_persistent_tmpfs failed: {e:?}");
        return 1;
    }

    if let Err(e) = fs::write(paths.lower.join("testfile"), b"from-lower") {
        eprintln!("seed lowerdir failed: {e}");
        let _ = unmount_persistent_tmpfs(&paths);
        return 2;
    }

    let target =
        std::env::temp_dir().join(format!("nyth-test-overlay-target-{}", std::process::id()));
    let _ = fs::remove_dir_all(&target);
    if fs::create_dir_all(&target).is_err() {
        eprintln!("create target dir failed");
        let _ = unmount_persistent_tmpfs(&paths);
        return 3;
    }

    if let Err(e) = mount_overlay(&paths, &target) {
        eprintln!("mount_overlay failed: {e:?}");
        let _ = unmount_persistent_tmpfs(&paths);
        let _ = fs::remove_dir_all(&target);
        return 4;
    }

    let result = check_overlay_contents(&target, &paths.upper);

    umount_path(&target);
    let _ = unmount_persistent_tmpfs(&paths);
    let _ = fs::remove_dir_all(&target);

    result
}

fn check_overlay_contents(target: &Path, upper: &Path) -> i32 {
    let seen = match fs::read(target.join("testfile")) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("reading through overlay failed: {e}");
            return 5;
        }
    };
    if seen != b"from-lower" {
        eprintln!("lowerdir content did not surface through overlay");
        return 6;
    }

    if let Err(e) = fs::write(target.join("newfile"), b"from-mount") {
        eprintln!("write through overlay failed: {e}");
        return 7;
    }

    match fs::read(upper.join("newfile")) {
        Ok(bytes) if bytes == b"from-mount" => 0,
        Ok(_) => {
            eprintln!("upperdir file had unexpected content");
            8
        }
        Err(e) => {
            eprintln!("write did not land in upperdir: {e}");
            9
        }
    }
}

#[test]
fn resolve_watched_paths_binds_dereferenced_store_target_not_the_symlink() {
    support::run_in_fork(run_resolve_in_child);
}

fn run_resolve_in_child() -> i32 {
    let name = format!("nyth-test-watched-{}", std::process::id());
    let paths = NythPaths::for_user(&name);

    if let Err(e) = provision_persistent_tmpfs(&paths) {
        if let OverlayError::PersistentTmpfsFailed { errno } = e {
            if is_permission_denied(errno) {
                return 0;
            }
        }
        eprintln!("provision_persistent_tmpfs failed: {e:?}");
        return 1;
    }

    // Stands in for a real /nix/store output: a plain, read-only file HM's symlink points at
    let fake_store_target = paths.root.join("fake-store-gitconfig");
    if fs::write(&fake_store_target, b"from-nix-store").is_err() {
        eprintln!("failed to seed fake store file");
        let _ = unmount_persistent_tmpfs(&paths);
        return 2;
    }
    // What Home Manager actually leaves in $HOME: an absolute symlink into the store
    if symlink(&fake_store_target, paths.home_snapshot.join(".gitconfig")).is_err() {
        eprintln!("failed to create symlink the way home-manager would");
        let _ = unmount_persistent_tmpfs(&paths);
        return 3;
    }

    let watched = match RelativeHomePath::new(".gitconfig") {
        Ok(path) => path,
        Err(e) => {
            eprintln!("RelativeHomePath::new failed: {e}");
            let _ = unmount_persistent_tmpfs(&paths);
            return 4;
        }
    };

    if let Err(e) = resolve_watched_paths(&paths, std::slice::from_ref(&watched)) {
        eprintln!("resolve_watched_paths failed: {e:?}");
        let _ = unmount_persistent_tmpfs(&paths);
        return 5;
    }

    let result = check_lower_entry(&paths.lower.join(".gitconfig"));
    let _ = unmount_persistent_tmpfs(&paths);
    result
}

fn check_lower_entry(lower_entry: &Path) -> i32 {
    let metadata = match fs::symlink_metadata(lower_entry) {
        Ok(metadata) => metadata,
        Err(e) => {
            eprintln!("stat on lower entry failed: {e}");
            return 6;
        }
    };
    if metadata.file_type().is_symlink() {
        eprintln!("lower/.gitconfig is still a symlink - write-through would escape the overlay");
        return 7;
    }

    match fs::read(lower_entry) {
        Ok(bytes) if bytes == b"from-nix-store" => 0,
        Ok(_) => {
            eprintln!("lower/.gitconfig content did not match the dereferenced store target");
            8
        }
        Err(e) => {
            eprintln!("reading through the resolved bind mount failed: {e}");
            9
        }
    }
}
