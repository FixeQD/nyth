use std::path::Path;

use nyth::config::{RelativeHomePath, RelativeHomePathError, parse_nyth_toml};
use nyth::error::ConfigInvalidReason;

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

const EXAMPLE_TOML: &str = r#"
[meta]
name = "minimal"
version = "0.1.0"

[env]
XDG_CURRENT_DESKTOP = "Hyprland"
EDITOR = "nvim"

[modules.hyprland]
source = "./dotfiles/hypr"
target = ".config/hypr"
on_change = "hyprctl reload"

[modules.git]
source = "./dotfiles/git/gitconfig"
target = ".gitconfig"
"#;

#[test]
fn parses_env_and_modules_from_example_config() {
    let config = parse_nyth_toml(EXAMPLE_TOML).expect("example config is valid");

    assert_eq!(config.env.get("EDITOR").map(String::as_str), Some("nvim"));
    assert_eq!(config.modules.len(), 2);

    // BTreeMap on the raw side sorts by module name: "git" before "hyprland"
    let (first_name, first_module) = &config.modules[0];
    assert_eq!(first_name, "git");
    assert_eq!(first_module.target.as_path(), Path::new(".gitconfig"));
    assert!(first_module.on_change.is_none());

    let (_, hyprland) = &config.modules[1];
    assert_eq!(hyprland.target.as_path(), Path::new(".config/hypr"));
    assert_eq!(
        hyprland.on_change.as_ref().map(|c| c.0.as_str()),
        Some("hyprctl reload")
    );
}

#[test]
fn config_with_no_modules_table_is_valid() {
    let config = parse_nyth_toml("[env]\nEDITOR = \"nvim\"\n").expect("modules are optional");
    assert!(config.modules.is_empty());
}

#[test]
fn rejects_module_target_escaping_home() {
    let toml = r#"
        [modules.evil]
        source = "./dotfiles/evil"
        target = "../../etc/passwd"
    "#;

    match parse_nyth_toml(toml) {
        Err(ConfigInvalidReason::InvalidTargetPath { module }) => assert_eq!(module, "evil"),
        other => panic!("expected InvalidTargetPath, got {other:?}"),
    }
}

#[test]
fn rejects_malformed_toml() {
    match parse_nyth_toml("this is not [ valid toml") {
        Err(ConfigInvalidReason::TomlParseFailed { .. }) => {}
        other => panic!("expected TomlParseFailed, got {other:?}"),
    }
}
