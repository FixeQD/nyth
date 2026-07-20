use std::ffi::{CString, OsString};
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::fd::OwnedFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::os::unix::fs::DirBuilderExt;
use std::path::Path;

use rustix::fs::CWD;
use rustix::mount::{
    FsMountFlags, FsOpenFlags, MountAttrFlags, MoveMountFlags, fsconfig_create,
    fsconfig_set_string, fsmount, fsopen, move_mount,
};

use crate::error::OverlayError;
use crate::sys::errno;
use crate::sys::paths::NythPaths;

/// Whether the overlay is currently mounted over a given target `$HOME`
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayState {
    Mounted,
    NotMounted,
}

/// Scans `/proc/self/mountinfo` for a mount point exactly at `home`
pub fn current_overlay_state(home: &Path) -> Result<OverlayState, OverlayError> {
    let file =
        fs::File::open("/proc/self/mountinfo").map_err(|e| OverlayError::StateCheckFailed {
            message: e.to_string(),
        })?;

    for line in BufReader::new(file).lines() {
        let line = line.map_err(|e| OverlayError::StateCheckFailed {
            message: e.to_string(),
        })?;
        // mountinfo(5): "... mount_id parent_id major:minor root mount_point ..."
        if let Some(mount_point) = line.split_whitespace().nth(4) {
            if Path::new(mount_point) == home {
                return Ok(OverlayState::Mounted);
            }
        }
    }
    Ok(OverlayState::NotMounted)
}

/// Sets up `/run/nyth/<name>/` as a persistent, root-owned tmpfs with the 4 subdirectories (`lower/`, `home-snapshot/`, `upper/`, `work/`)
pub fn provision_persistent_tmpfs(paths: &NythPaths) -> Result<(), OverlayError> {
    create_root_dir(&paths.root)?;
    mount_tmpfs(&paths.root)?;

    for dir in [
        &paths.lower,
        &paths.home_snapshot,
        &paths.upper,
        &paths.work,
    ] {
        create_dir_idempotent(dir)?;
    }
    Ok(())
}

fn create_root_dir(root: &Path) -> Result<(), OverlayError> {
    // `/run/nyth` doesn't necessarily exist yet - recursive() so this doesn't fail with ENOENT the first time it runs on a given machin
    match fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(root)
    {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(OverlayError::PersistentTmpfsFailed {
            errno: e.raw_os_error().unwrap_or(0),
        }),
    }
}

fn create_dir_idempotent(path: &Path) -> Result<(), OverlayError> {
    match fs::create_dir(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(e) => Err(OverlayError::PersistentTmpfsFailed {
            errno: e.raw_os_error().unwrap_or(0),
        }),
    }
}

fn mount_tmpfs(path: &Path) -> Result<(), OverlayError> {
    let target = to_cstring(path);
    let fstype = c"tmpfs";

    let ret = unsafe {
        libc::mount(
            fstype.as_ptr(),
            target.as_ptr(),
            fstype.as_ptr(),
            libc::MS_NOSUID | libc::MS_NODEV,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        return Err(OverlayError::PersistentTmpfsFailed { errno: errno() });
    }
    Ok(())
}

fn to_cstring(path: impl AsRef<Path>) -> CString {
    CString::new(path.as_ref().as_os_str().as_bytes()).expect("path has no interior NUL")
}

/// Read-only bind mount of the target's $HOME as it was before the overlay goes on top
pub fn mount_home_snapshot(home: &Path, paths: &NythPaths) -> Result<(), OverlayError> {
    bind_mount_readonly(home, &paths.home_snapshot)
        .map_err(|errno| OverlayError::HomeSnapshotFailed { errno })
}

/// Bind mount + remount read-only in one call, raw errno on failure
fn bind_mount_readonly(source: &Path, target: &Path) -> Result<(), i32> {
    bind_mount(source, target)?;
    remount_readonly(target)
}

fn bind_mount(source: &Path, target: &Path) -> Result<(), i32> {
    let ret = unsafe {
        libc::mount(
            to_cstring(source).as_ptr(),
            to_cstring(target).as_ptr(),
            std::ptr::null(),
            libc::MS_BIND | libc::MS_NOSUID | libc::MS_NODEV,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        return Err(errno());
    }
    Ok(())
}

// Two-step bind+remount (MS_RDONLY ignored on initial MS_BIND).
// Flags repeated on both calls: if source's host mount already has them locked (e.g. /tmp nosuid,nodev), omitting them here gets EPERM (mount_namespaces(7))
fn remount_readonly(target: &Path) -> Result<(), i32> {
    let ret = unsafe {
        libc::mount(
            std::ptr::null(),
            to_cstring(target).as_ptr(),
            std::ptr::null(),
            libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY | libc::MS_NOSUID | libc::MS_NODEV,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        return Err(errno());
    }
    Ok(())
}

/// Copies Home Manager's fully-merged `home-files` derivation (the same `$out` HM itself would symlink into `$HOME`
pub fn materialize_home_files(
    paths: &NythPaths,
    home_files: &Path,
    uid: u32,
    gid: u32,
) -> Result<(), OverlayError> {
    copy_tree_dereferenced(home_files, &paths.lower).map_err(|e| {
        OverlayError::HomeFilesMaterializeFailed {
            path: home_files.to_path_buf(),
            errno: e.raw_os_error().unwrap_or(0),
        }
    })?;

    chown_tree(&paths.lower, uid, gid)
}

/// Recursively copies `source` into `destination`, following symlinks
fn copy_tree_dereferenced(source: &Path, destination: &Path) -> std::io::Result<()> {
    let metadata = fs::metadata(source)?; // follows symlinks, unlike symlink_metadata

    if metadata.is_dir() {
        fs::create_dir_all(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy_tree_dereferenced(&entry.path(), &destination.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(source, destination).map(|_| ())
    }
}

/// `chown`s `path` and, if it's a directory, everything underneath it.
fn chown_tree(path: &Path, uid: u32, gid: u32) -> Result<(), OverlayError> {
    set_ownership(path, uid, gid)?;

    if path.is_dir() {
        let entries = fs::read_dir(path).map_err(|e| OverlayError::OwnershipFailed {
            path: path.to_path_buf(),
            errno: e.raw_os_error().unwrap_or(0),
        })?;
        for entry in entries {
            let entry = entry.map_err(|e| OverlayError::OwnershipFailed {
                path: path.to_path_buf(),
                errno: e.raw_os_error().unwrap_or(0),
            })?;
            chown_tree(&entry.path(), uid, gid)?;
        }
    }
    Ok(())
}

pub fn mount_overlay(paths: &NythPaths, target: &Path) -> Result<(), OverlayError> {
    let fs_fd = open_overlay_fs()?;

    set_lowerdir(&fs_fd, &paths.lower, &paths.home_snapshot)?;
    set_dir_option(&fs_fd, "upperdir", &paths.upper)?;
    set_dir_option(&fs_fd, "workdir", &paths.work)?;
    fsconfig_create(&fs_fd).map_err(|e| mount_failed(target, e))?;

    let mount_fd = fsmount(
        &fs_fd,
        FsMountFlags::FSMOUNT_CLOEXEC,
        MountAttrFlags::empty(),
    )
    .map_err(|e| mount_failed(target, e))?;

    move_mount(
        &mount_fd,
        "",
        CWD,
        target,
        MoveMountFlags::MOVE_MOUNT_F_EMPTY_PATH,
    )
    .map_err(|e| mount_failed(target, e))
}

/// ENOSYS = no new mount API (kernel < 5.2); EOPNOTSUPP/ENODEV = overlay module not loaded
fn open_overlay_fs() -> Result<OwnedFd, OverlayError> {
    fsopen("overlay", FsOpenFlags::FSOPEN_CLOEXEC).map_err(|e| match e {
        rustix::io::Errno::NOSYS | rustix::io::Errno::OPNOTSUPP | rustix::io::Errno::NODEV => {
            OverlayError::OverlayApiUnsupported {
                errno: e.raw_os_error(),
            }
        }
        other => mount_failed(Path::new("overlay"), other),
    })
}

fn set_lowerdir(fs_fd: &OwnedFd, lower: &Path, home_snapshot: &Path) -> Result<(), OverlayError> {
    let mut value = lower.as_os_str().as_bytes().to_vec();
    value.push(b':');
    value.extend_from_slice(home_snapshot.as_os_str().as_bytes());
    let value = OsString::from_vec(value);

    fsconfig_set_string(fs_fd, "lowerdir", value.as_os_str()).map_err(|e| mount_failed(lower, e))
}

fn set_dir_option(fs_fd: &OwnedFd, key: &str, dir: &Path) -> Result<(), OverlayError> {
    fsconfig_set_string(fs_fd, key, dir.as_os_str()).map_err(|e| mount_failed(dir, e))
}

fn mount_failed(target: &Path, e: rustix::io::Errno) -> OverlayError {
    OverlayError::MountFailed {
        target: target.to_path_buf(),
        errno: e.raw_os_error(),
    }
}

/// Unmounts the overlay at `target` and the read-only home snapshot underneath it
pub fn unmount_overlay_and_snapshot(target: &Path, paths: &NythPaths) -> Result<(), OverlayError> {
    unmount_one(target)?;
    unmount_one(&paths.home_snapshot)
}

/// Additionally tears down the persistent tmpfs itself (`nyth unmount --purge`): `upper`/`work` go with it
pub fn unmount_persistent_tmpfs(paths: &NythPaths) -> Result<(), OverlayError> {
    unmount_one(&paths.root)
}

fn unmount_one(target: &Path) -> Result<(), OverlayError> {
    let ret = unsafe { libc::umount2(to_cstring(target).as_ptr(), 0) };
    if ret != 0 {
        return Err(OverlayError::UnmountFailed {
            target: target.to_path_buf(),
            errno: errno(),
        });
    }
    Ok(())
}

/// `chown`s `path` to `uid`/`gid`. `upper`/`work` are created by root but need to be writable by the target user's own processes running inside the overlay
pub fn set_ownership(path: &Path, uid: u32, gid: u32) -> Result<(), OverlayError> {
    let ret = unsafe { libc::chown(to_cstring(path).as_ptr(), uid, gid) };
    if ret != 0 {
        return Err(OverlayError::OwnershipFailed {
            path: path.to_path_buf(),
            errno: errno(),
        });
    }
    Ok(())
}
