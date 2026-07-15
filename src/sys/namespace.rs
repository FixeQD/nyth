use std::ffi::CStr;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use crate::error::NamespaceError;

/// The real identity of whoever invoked nyth, read straight from the kernel and the passwd database
#[derive(Debug, Clone)]
pub struct CallerIdentity {
    pub uid: u32,
    pub gid: u32,
    pub home: PathBuf,
}

impl CallerIdentity {
    pub fn from_current_process() -> Result<Self, NamespaceError> {
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };
        let home = home_dir_for(uid)?;
        Ok(Self { uid, gid, home })
    }
}

/// Looks up the home directory for `uid` via getpwuid_r, retrying with a larger buffer on ERANGE
fn home_dir_for(uid: u32) -> Result<PathBuf, NamespaceError> {
    let mut buf_len: usize = 1024;

    loop {
        let mut buf = vec![0u8; buf_len];
        let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
        let mut result: *mut libc::passwd = std::ptr::null_mut();

        let ret = unsafe {
            libc::getpwuid_r(
                uid,
                &mut pwd,
                buf.as_mut_ptr() as *mut libc::c_char,
                buf.len(),
                &mut result,
            )
        };

        if ret == 0 {
            if result.is_null() {
                // Syscall succeeded but there is no passwd entry for this uid
                return Err(NamespaceError::HomeLookupFailed { uid, errno: 0 });
            }

            let home_cstr = unsafe { CStr::from_ptr(pwd.pw_dir) };
            return Ok(PathBuf::from(home_cstr.to_string_lossy().into_owned()));
        }

        if ret == libc::ERANGE {
            buf_len *= 2;
            continue;
        }

        return Err(NamespaceError::HomeLookupFailed { uid, errno: ret });
    }
}

/// A user + mount namespace the caller is running inside of
pub struct IsolatedSession {
    uid_map_written: bool,
}

pub fn enter_isolated_session(uid: u32, gid: u32) -> Result<IsolatedSession, NamespaceError> {
    unsafe {
        if libc::unshare(libc::CLONE_NEWUSER | libc::CLONE_NEWNS) != 0 {
            let err = errno();
            if err == libc::ENOSPC {
                // Common cause on NixOS: security.allowUserNamespaces = false, or a hardened profile disabling unprivileged user namespaces
                return Err(NamespaceError::UserNamespacesDisabled);
            }
            return Err(NamespaceError::UnshareFailed { errno: err });
        }
    }

    write_setgroups_deny()?;
    write_uid_gid_map(uid, gid)?;

    Ok(IsolatedSession {
        uid_map_written: true,
    })
}

fn write_setgroups_deny() -> Result<(), NamespaceError> {
    write_proc_self_file("/proc/self/setgroups", b"deny")
        .map_err(|errno| NamespaceError::SetgroupsWriteFailed { errno })
}

fn write_uid_gid_map(uid: u32, gid: u32) -> Result<(), NamespaceError> {
    write_proc_self_file("/proc/self/uid_map", format!("0 {uid} 1").as_bytes())
        .map_err(|errno| NamespaceError::UidMapWriteFailed { errno })?;
    write_proc_self_file("/proc/self/gid_map", format!("0 {gid} 1").as_bytes())
        .map_err(|errno| NamespaceError::UidMapWriteFailed { errno })
}

/// Opens `path` for writing and writes `contents` in one call
fn write_proc_self_file(path: &str, contents: &[u8]) -> Result<(), i32> {
    let mut file = OpenOptions::new()
        .write(true)
        .open(path)
        .map_err(|e| e.raw_os_error().unwrap_or(0))?;
    file.write_all(contents)
        .map_err(|e| e.raw_os_error().unwrap_or(0))
}

fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}
