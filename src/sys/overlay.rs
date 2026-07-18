use std::ffi::{CStr, CString, OsString};
use std::fs;
use std::os::fd::OwnedFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use rustix::fs::CWD;
use rustix::mount::{
    FsMountFlags, FsOpenFlags, MountAttrFlags, MoveMountFlags, fsconfig_create,
    fsconfig_set_string, fsmount, fsopen, move_mount,
};

use crate::config::RelativeHomePath;
use crate::error::OverlayError;
use crate::sys::errno;
use crate::sys::namespace::CallerIdentity;

/// Layout:
/// /tmp/.nyth-<uid>-<suffix>/{home-snapshot,upper,work}
/// Lives entirely inside the mount namespace from enter_isolated_session, gone once that namespace goes away
pub struct ScratchTmpfs {
    pub root: PathBuf,
    pub home_snapshot: PathBuf,
}

pub fn provision_scratch_tmpfs(identity: &CallerIdentity) -> Result<ScratchTmpfs, OverlayError> {
    let root = create_scratch_dir(identity.uid)?;
    mount_tmpfs(&root)?;

    Ok(ScratchTmpfs {
        home_snapshot: make_subdir(&root, "home-snapshot")?,
        root,
    })
}

// mkdtemp creates the dir atomically with mode 0700
fn create_scratch_dir(uid: u32) -> Result<PathBuf, OverlayError> {
    let template = format!("/tmp/.nyth-{uid}-XXXXXX\0");
    let mut buf = template.into_bytes();

    let result = unsafe { libc::mkdtemp(buf.as_mut_ptr() as *mut libc::c_char) };
    if result.is_null() {
        return Err(OverlayError::ScratchDirCreateFailed { errno: errno() });
    }

    let path_cstr = unsafe { CStr::from_ptr(buf.as_ptr() as *const libc::c_char) };
    Ok(PathBuf::from(std::ffi::OsString::from_vec(
        path_cstr.to_bytes().to_vec(),
    )))
}

fn mount_tmpfs(path: &PathBuf) -> Result<(), OverlayError> {
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
        return Err(OverlayError::ScratchTmpfsMountFailed { errno: errno() });
    }
    Ok(())
}

fn make_subdir(root: &PathBuf, name: &str) -> Result<PathBuf, OverlayError> {
    let path = root.join(name);
    let ret = unsafe { libc::mkdir(to_cstring(&path).as_ptr(), 0o700) };
    if ret != 0 {
        return Err(OverlayError::ScratchSubdirFailed {
            path,
            errno: errno(),
        });
    }
    Ok(path)
}

fn to_cstring(path: impl AsRef<Path>) -> CString {
    CString::new(path.as_ref().as_os_str().as_bytes()).expect("path has no interior NUL")
}

/// Read-only bind mount of $HOME as it was before the overlay goes on top
/// Btw without this, mounting the overlay straight onto $HOME would shadow everything outside the configured modules for the whole session
pub fn mount_home_snapshot(home: &Path, scratch: &ScratchTmpfs) -> Result<(), OverlayError> {
    bind_mount_readonly(home, &scratch.home_snapshot)
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

/// For each watched path, dereferences the Home Manager-generated symlink at `home_snapshot/<path>` down to its real target in `/nix/store`, then bind-mounts that target read-only at `lower/<path>`
pub fn resolve_watched_paths(
    scratch: &ScratchTmpfs,
    lower: &Path,
    watched_paths: &[RelativeHomePath],
) -> Result<(), OverlayError> {
    for path in watched_paths {
        resolve_one_watched_path(scratch, lower, path)?;
    }
    Ok(())
}

fn resolve_one_watched_path(
    scratch: &ScratchTmpfs,
    lower: &Path,
    path: &RelativeHomePath,
) -> Result<(), OverlayError> {
    let home_managed_entry = scratch.home_snapshot.join(path.as_path());
    let store_target =
        fs::canonicalize(&home_managed_entry).map_err(|e| watched_path_unresolved(path, &e))?;

    let lower_target = lower.join(path.as_path());
    create_bind_mountpoint(&lower_target, &store_target)
        .map_err(|e| watched_path_unresolved(path, &e))?;

    bind_mount_readonly(&store_target, &lower_target).map_err(|errno| {
        OverlayError::WatchedPathUnresolved {
            path: path.as_path().to_path_buf(),
            errno,
        }
    })
}

fn watched_path_unresolved(path: &RelativeHomePath, e: &std::io::Error) -> OverlayError {
    OverlayError::WatchedPathUnresolved {
        path: path.as_path().to_path_buf(),
        errno: e.raw_os_error().unwrap_or(0),
    }
}

/// Bind mount targets must already exist and match the source's type
fn create_bind_mountpoint(lower_target: &Path, store_target: &Path) -> std::io::Result<()> {
    if let Some(parent) = lower_target.parent() {
        fs::create_dir_all(parent)?;
    }

    if store_target.is_dir() {
        fs::create_dir(lower_target)
    } else {
        fs::File::create(lower_target).map(|_| ())
    }
}

/// Mounts overlayfs at `target`: lowerdir = `lower` over `scratch.home_snapshot`, upperdir/workdir from `scratch`
pub fn mount_overlay(
    lower: &Path,
    upper: &Path,
    work: &Path,
    scratch: &ScratchTmpfs,
    target: &Path,
) -> Result<(), OverlayError> {
    let fs_fd = open_overlay_fs()?;

    set_lowerdir(&fs_fd, lower, &scratch.home_snapshot)?;
    set_dir_option(&fs_fd, "upperdir", upper)?;
    set_dir_option(&fs_fd, "workdir", work)?;
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
