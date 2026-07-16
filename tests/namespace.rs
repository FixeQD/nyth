mod support;

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

    support::run_in_fork(
        || match enter_isolated_session(identity.uid, identity.gid) {
            Ok(_) => 0,
            Err(NamespaceError::UserNamespacesDisabled) => 0,
            Err(_) => 1,
        },
    );
}
