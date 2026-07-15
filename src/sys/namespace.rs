use std::ffi::CStr;
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
