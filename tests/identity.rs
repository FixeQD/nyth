use nyth::error::IdentityError;
use nyth::sys::identity::{TargetIdentity, require_real_root};

#[test]
fn from_username_resolves_root_via_getpwnam_r() {
    let identity = TargetIdentity::from_username("root").expect("root always exists");
    assert_eq!(identity.uid, 0);
    assert!(!identity.home.as_os_str().is_empty());
    assert!(!identity.shell.as_os_str().is_empty());
}

#[test]
fn from_username_reports_unknown_users_as_a_typed_error() {
    let err = TargetIdentity::from_username("nyth-test-user-that-does-not-exist-hopefully")
        .expect_err("this user should not exist");
    assert!(matches!(err, IdentityError::UserNotFound { .. }));
}

#[test]
fn require_real_root_matches_geteuid() {
    let result = require_real_root();
    if unsafe { libc::geteuid() } == 0 {
        assert!(result.is_ok());
    } else {
        assert!(matches!(result, Err(IdentityError::NotRunningAsRoot)));
    }
}
