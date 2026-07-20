use nyth::cli::mount::parse_mount_args;
use nyth::cli::unmount::parse_unmount_args;

fn args(raw: &[&str]) -> Vec<String> {
    raw.iter().map(|s| s.to_string()).collect()
}

#[test]
fn mount_parses_for_user_and_home_files() {
    let parsed = parse_mount_args(&args(&[
        "--for-user",
        "user",
        "--home-files",
        "/nix/store/abc-home-manager-files",
    ]))
    .expect("valid argv should parse");

    assert_eq!(parsed.for_user, "user");
    assert_eq!(
        parsed.home_files,
        std::path::PathBuf::from("/nix/store/abc-home-manager-files")
    );
}

#[test]
fn mount_without_for_user_is_an_error() {
    let err = parse_mount_args(&args(&[
        "--home-files",
        "/nix/store/abc-home-manager-files",
    ]))
    .unwrap_err();
    assert_eq!(err.to_string(), "--for-user <name> is required");
}

#[test]
fn mount_without_home_files_is_an_error() {
    let err = parse_mount_args(&args(&["--for-user", "user"])).unwrap_err();
    assert_eq!(err.to_string(), "--home-files <path> is required");
}

#[test]
fn mount_rejects_unknown_flags() {
    let err = parse_mount_args(&args(&["--for-user", "user", "--bogus"])).unwrap_err();
    assert_eq!(
        err.to_string(),
        "unexpected argument '--bogus', expected --for-user or --home-files"
    );
}

#[test]
fn unmount_parses_for_user_and_purge() {
    let parsed = parse_unmount_args(&args(&["--for-user", "user", "--purge"])).expect("valid argv");
    assert_eq!(parsed.for_user, "user");
    assert!(parsed.purge);
}

#[test]
fn unmount_purge_defaults_to_false() {
    let parsed = parse_unmount_args(&args(&["--for-user", "user"])).expect("valid argv");
    assert!(!parsed.purge);
}

#[test]
fn unmount_without_for_user_is_an_error() {
    let err = parse_unmount_args(&args(&["--purge"])).unwrap_err();
    assert_eq!(err.to_string(), "--for-user <name> is required");
}
