use std::ffi::{CStr, CString, OsString};
use std::os::fd::OwnedFd;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use rustix::fs::CWD;
use rustix::mount::{
    FsMountFlags, FsOpenFlags, MountAttrFlags, MoveMountFlags, fsconfig_create,
    fsconfig_set_string, fsmount, fsopen, move_mount,
};

use crate::error::OverlayError;
use crate::sys::errno;
use crate::sys::namespace::CallerIdentity;

/// Layout:
/// /tmp/.nyth-<uid>-<suffix>/{lower,home-snapshot,upper,work}
/// Lives entirely inside the mount namespace from enter_isolated_session, gone once that namespace goes away
pub struct ScratchTmpfs {
    pub root: PathBuf,
    pub lower: PathBuf,
    pub home_snapshot: PathBuf,
    pub upper: PathBuf,
    pub work: PathBuf,
}

pub fn provision_scratch_tmpfs(identity: &CallerIdentity) -> Result<ScratchTmpfs, OverlayError> {
    let root = create_scratch_dir(identity.uid)?;
    mount_tmpfs(&root)?;

    Ok(ScratchTmpfs {
        lower: make_subdir(&root, "lower")?,
        home_snapshot: make_subdir(&root, "home-snapshot")?,
        upper: make_subdir(&root, "upper")?,
        work: make_subdir(&root, "work")?,
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
    bind_mount(home, &scratch.home_snapshot)?;
    remount_readonly(&scratch.home_snapshot)
}

fn bind_mount(source: &Path, target: &Path) -> Result<(), OverlayError> {
    let ret = unsafe {
        libc::mount(
            to_cstring(source).as_ptr(),
            to_cstring(target).as_ptr(),
            std::ptr::null(),
            libc::MS_BIND,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        return Err(OverlayError::HomeSnapshotFailed { errno: errno() });
    }
    Ok(())
}

/// MS_RDONLY is ignored on the initial MS_BIND call, a read-only bind mount always needs this second MS_REMOUNT pass
fn remount_readonly(target: &Path) -> Result<(), OverlayError> {
    let ret = unsafe {
        libc::mount(
            std::ptr::null(),
            to_cstring(target).as_ptr(),
            std::ptr::null(),
            libc::MS_BIND | libc::MS_REMOUNT | libc::MS_RDONLY,
            std::ptr::null(),
        )
    };
    if ret != 0 {
        return Err(OverlayError::HomeSnapshotFailed { errno: errno() });
    }
    Ok(())
}

/// Mounts overlayfs at `target`: lowerdir = modules over home-snapshot, upperdir/workdir from `scratch`
pub fn mount_overlay(scratch: &ScratchTmpfs, target: &Path) -> Result<(), OverlayError> {
    let fs_fd = open_overlay_fs()?;

    set_lowerdir(&fs_fd, &scratch.lower, &scratch.home_snapshot)?;
    set_dir_option(&fs_fd, "upperdir", &scratch.upper)?;
    set_dir_option(&fs_fd, "workdir", &scratch.work)?;
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
