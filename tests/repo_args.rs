use std::path::PathBuf;

use nyth::cli::status::parse_repo_args;

fn args(raw: &[&str]) -> Vec<String> {
    raw.iter().map(|s| s.to_string()).collect()
}

#[test]
fn parses_for_user_repo_root_and_both_path_lists() {
    let parsed = parse_repo_args(&args(&[
        "--for-user",
        "user",
        "--repo-root",
        "/home/fixeq/nix-config",
        "--repo-backed",
        ".gitconfig",
        "--repo-backed",
        ".config/hypr",
        "--generated",
        ".zshrc",
    ]))
    .expect("valid argv should parse");

    assert_eq!(parsed.for_user, "user");
    assert_eq!(
        parsed.repo_root,
        Some(PathBuf::from("/home/fixeq/nix-config"))
    );
    assert_eq!(
        parsed
            .repo_backed_paths
            .iter()
            .map(|p| p.as_path().to_path_buf())
            .collect::<Vec<_>>(),
        vec![PathBuf::from(".gitconfig"), PathBuf::from(".config/hypr")]
    );
    assert_eq!(
        parsed
            .generated_paths
            .iter()
            .map(|p| p.as_path().to_path_buf())
            .collect::<Vec<_>>(),
        vec![PathBuf::from(".zshrc")]
    );
}

#[test]
fn missing_for_user_is_an_error() {
    let err = parse_repo_args(&args(&["--repo-backed", ".gitconfig"])).unwrap_err();
    assert_eq!(err.to_string(), "--for-user <name> is required");
}

#[test]
fn for_user_only_parses_to_defaults_repo_root_falls_back_to_cwd_on_into_repo() {
    let parsed = parse_repo_args(&args(&["--for-user", "user"])).expect("--for-user is enough");

    assert_eq!(parsed.for_user, "user");
    assert_eq!(parsed.repo_root, None);
    assert!(parsed.repo_backed_paths.is_empty());
    assert!(parsed.generated_paths.is_empty());

    let repo = parsed.into_repo();
    assert_eq!(repo.root, PathBuf::from("."));
}

#[test]
fn missing_flag_value_is_an_error_not_a_panic() {
    let err = parse_repo_args(&args(&["--for-user", "user", "--repo-backed"])).unwrap_err();
    assert_eq!(err.to_string(), "--repo-backed requires a value");
}

#[test]
fn invalid_relative_path_is_reported_with_which_flag_it_came_from() {
    let err = parse_repo_args(&args(&[
        "--for-user",
        "user",
        "--generated",
        "/absolute/not/relative",
    ]))
    .unwrap_err();
    assert!(err.to_string().starts_with("invalid --generated "));
}

#[test]
fn unknown_flag_is_rejected() {
    let err = parse_repo_args(&args(&["--for-user", "user", "--nonsense"])).unwrap_err();
    assert_eq!(
        err.to_string(),
        "unexpected argument '--nonsense', expected --for-user, --repo-root, --repo-backed, or --generated"
    );
}
