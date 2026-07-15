use std::process::exit;

use nyth::error::NamespaceError;
use nyth::sys::namespace::{CallerIdentity, enter_isolated_session};

#[test]
fn caller_identity_matches_raw_syscalls() {
    let identity = CallerIdentity::from_current_process().expect("syscalls don't fail");
    assert_eq!(identity.uid, unsafe { libc::getuid() });
    assert_eq!(identity.gid, unsafe { libc::getgid() });
    assert!(!identity.home.as_os_str().is_empty());
}

#[test]
fn enter_isolated_session_succeeds_or_is_disabled() {
    let identity = CallerIdentity::from_current_process().expect("syscalls don't fail");
    let (uid, gid) = (identity.uid, identity.gid);

    match unsafe { libc::fork() } {
        -1 => panic!("fork failed"),
        0 => {
            let code = match enter_isolated_session(uid, gid) {
                Ok(_) => 0,
                Err(NamespaceError::UserNamespacesDisabled) => 0,
                Err(_) => 1,
            };
            exit(code);
        }
        child_pid => {
            let mut status = 0;
            unsafe { libc::waitpid(child_pid, &mut status, 0) };
            assert!(libc::WIFEXITED(status), "child did not exit normally");
            assert_eq!(libc::WEXITSTATUS(status), 0, "unexpected error in child");
        }
    }
}
