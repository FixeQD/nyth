use std::ffi::{CStr, CString};
use std::path::PathBuf;

use crate::error::IdentityError;

/// The identity nyth acts on behalf of - resolved from the `--for-user <name>` argument via `getpwnam_r`
#[derive(Debug, Clone)]
pub struct TargetIdentity {
    pub uid: u32,
    pub gid: u32,
    pub home: PathBuf,
    pub shell: PathBuf,
}

impl TargetIdentity {
    /// `name` is whatever came in on `--for-user`
    pub fn from_username(name: &str) -> Result<Self, IdentityError> {
        let cname = CString::new(name).map_err(|_| IdentityError::UserNotFound {
            name: name.to_string(),
        })?;

        let mut buf_len: usize = 1024;

        loop {
            let mut buf = vec![0u8; buf_len];
            let mut pwd: libc::passwd = unsafe { std::mem::zeroed() };
            let mut result: *mut libc::passwd = std::ptr::null_mut();

            let ret = unsafe {
                libc::getpwnam_r(
                    cname.as_ptr(),
                    &mut pwd,
                    buf.as_mut_ptr() as *mut libc::c_char,
                    buf.len(),
                    &mut result,
                )
            };

            if ret == 0 {
                if result.is_null() {
                    // Syscall succeeded but there is no passwd entry for this name
                    return Err(IdentityError::UserNotFound {
                        name: name.to_string(),
                    });
                }

                let home_cstr = unsafe { CStr::from_ptr(pwd.pw_dir) };
                let shell_cstr = unsafe { CStr::from_ptr(pwd.pw_shell) };
                return Ok(Self {
                    uid: pwd.pw_uid,
                    gid: pwd.pw_gid,
                    home: PathBuf::from(home_cstr.to_string_lossy().into_owned()),
                    shell: PathBuf::from(shell_cstr.to_string_lossy().into_owned()),
                });
            }

            if ret == libc::ERANGE {
                buf_len *= 2;
                continue;
            }

            return Err(IdentityError::HomeLookupFailed {
                name: name.to_string(),
                errno: ret,
            });
        }
    }
}

/// The only precondition-check nyth runs on its own process
pub fn require_real_root() -> Result<(), IdentityError> {
    if unsafe { libc::geteuid() } != 0 {
        return Err(IdentityError::NotRunningAsRoot);
    }
    Ok(())
}
