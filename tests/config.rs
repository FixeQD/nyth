use std::path::Path;

use nyth::config::{RelativeHomePath, RelativeHomePathError};

#[test]
fn accepts_plain_relative_path() {
    let path = RelativeHomePath::new(".config/nvim/init.lua").expect("valid relative path");
    assert_eq!(path.as_path(), Path::new(".config/nvim/init.lua"));
}

#[test]
fn rejects_absolute_path() {
    match RelativeHomePath::new("/etc/passwd") {
        Err(RelativeHomePathError::AbsolutePath(_)) => {}
        other => panic!("expected AbsolutePath, got {other:?}"),
    }
}

#[test]
fn rejects_parent_dir_escape() {
    match RelativeHomePath::new("../../etc/passwd") {
        Err(RelativeHomePathError::EscapesHome(_)) => {}
        other => panic!("expected EscapesHome, got {other:?}"),
    }
}

#[test]
fn rejects_parent_dir_in_the_middle() {
    // Not just a leading "..": "a/../../b" still resolves outside $HOME
    match RelativeHomePath::new("a/../../b") {
        Err(RelativeHomePathError::EscapesHome(_)) => {}
        other => panic!("expected EscapesHome, got {other:?}"),
    }
}
