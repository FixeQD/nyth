use std::fs;
use std::path::PathBuf;

use nyth::build::build_into;
use nyth::error::NythError;

struct Workspace {
    root: PathBuf,
}

impl Workspace {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("nyth-build-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create workspace root");
        Self { root }
    }

    fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).expect("create parent dirs");
        fs::write(path, contents).expect("write workspace file");
    }

    fn config_path(&self) -> PathBuf {
        self.root.join("nyth.toml")
    }

    fn lower_path(&self) -> PathBuf {
        self.root.join("lower")
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn copies_directory_and_single_file_modules() {
    let ws = Workspace::new("copies");
    ws.write("dotfiles/hypr/hyprland.conf", "monitor=,preferred,auto,1");
    ws.write("dotfiles/git/gitconfig", "[user]\nname = test");
    ws.write(
        "nyth.toml",
        r#"
            [modules.hyprland]
            source = "./dotfiles/hypr"
            target = ".config/hypr"

            [modules.git]
            source = "./dotfiles/git/gitconfig"
            target = ".gitconfig"
        "#,
    );

    build_into(&ws.config_path(), &ws.lower_path()).expect("build should succeed");

    let hyprland = fs::read_to_string(ws.lower_path().join(".config/hypr/hyprland.conf"))
        .expect("hyprland module copied");
    assert_eq!(hyprland, "monitor=,preferred,auto,1");

    let gitconfig =
        fs::read_to_string(ws.lower_path().join(".gitconfig")).expect("git module copied");
    assert_eq!(gitconfig, "[user]\nname = test");
}

#[test]
fn rebuild_removes_modules_dropped_from_config() {
    let ws = Workspace::new("rebuild");
    ws.write("dotfiles/git/gitconfig", "[user]\nname = test");
    ws.write(
        "nyth.toml",
        r#"
            [modules.git]
            source = "./dotfiles/git/gitconfig"
            target = ".gitconfig"
        "#,
    );
    build_into(&ws.config_path(), &ws.lower_path()).expect("first build should succeed");
    assert!(ws.lower_path().join(".gitconfig").exists());

    // Rewrite config with the module gone: rebuilding must not leave stale files.
    ws.write("nyth.toml", "[env]\nEDITOR = \"nvim\"\n");
    build_into(&ws.config_path(), &ws.lower_path()).expect("second build should succeed");

    assert!(!ws.lower_path().join(".gitconfig").exists());
}

#[test]
fn missing_module_source_is_a_module_build_error() {
    let ws = Workspace::new("missing-source");
    ws.write(
        "nyth.toml",
        r#"
            [modules.ghost]
            source = "./dotfiles/does-not-exist"
            target = ".config/ghost"
        "#,
    );

    match build_into(&ws.config_path(), &ws.lower_path()) {
        Err(NythError::ModuleBuildFailed { module, .. }) => assert_eq!(module, "ghost"),
        other => panic!("expected ModuleBuildFailed, got {other:?}"),
    }
}

#[test]
fn missing_config_file_is_a_config_invalid_error() {
    let ws = Workspace::new("missing-config");

    match build_into(&ws.config_path(), &ws.lower_path()) {
        Err(NythError::ConfigInvalid { .. }) => {}
        other => panic!("expected ConfigInvalid, got {other:?}"),
    }
}
