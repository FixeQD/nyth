use std::fs;
use std::path::PathBuf;

use nyth::commit::commit_into;
use nyth::sys::paths::NythPaths;

// Same throwaway-workspace pattern as tests/build.rs
struct Workspace {
    root: PathBuf,
}

impl Workspace {
    fn new(name: &str) -> Self {
        let root =
            std::env::temp_dir().join(format!("nyth-commit-test-{name}-{}", std::process::id()));
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

    fn paths(&self) -> NythPaths {
        NythPaths {
            lower: self.root.join("state/lower"),
            upper: self.root.join("state/upper"),
            work: self.root.join("state/work"),
            root: self.root.join("state"),
        }
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[test]
fn commit_writes_module_modified_file_back_to_repo_source() {
    let ws = Workspace::new("basic");
    ws.write("dotfiles/gitconfig", "[user]\nname = old");
    ws.write(
        "nyth.toml",
        "[modules.git]\nsource = \"./dotfiles/gitconfig\"\ntarget = \".gitconfig\"\n",
    );
    let paths = ws.paths();
    ws.write("state/upper/.gitconfig", "[user]\nname = new");

    let report = commit_into(&ws.config_path(), &paths).expect("commit should succeed");

    assert_eq!(report.applied, vec![ws.root.join("dotfiles/gitconfig")]);
    let written = fs::read_to_string(ws.root.join("dotfiles/gitconfig")).expect("read repo file");
    assert_eq!(written, "[user]\nname = new");
}

#[test]
fn commit_ignores_untracked_files() {
    let ws = Workspace::new("untracked");
    ws.write("nyth.toml", "[env]\nEDITOR = \"nvim\"\n");
    let paths = ws.paths();
    ws.write("state/upper/random-state.db", "");

    let report = commit_into(&ws.config_path(), &paths).expect("commit should succeed");

    assert!(report.applied.is_empty());
}

#[test]
fn commit_writes_nested_directory_module_file_to_right_subpath() {
    let ws = Workspace::new("nested");
    ws.write("dotfiles/hypr/hyprland.conf", "monitor=old");
    ws.write(
        "nyth.toml",
        "[modules.hyprland]\nsource = \"./dotfiles/hypr\"\ntarget = \".config/hypr\"\n",
    );
    let paths = ws.paths();
    ws.write("state/upper/.config/hypr/hyprland.conf", "monitor=new");

    let report = commit_into(&ws.config_path(), &paths).expect("commit should succeed");

    assert_eq!(
        report.applied,
        vec![ws.root.join("dotfiles/hypr/hyprland.conf")]
    );
    let written =
        fs::read_to_string(ws.root.join("dotfiles/hypr/hyprland.conf")).expect("read repo file");
    assert_eq!(written, "monitor=new");
}

#[test]
fn commit_with_nothing_in_upper_applies_nothing() {
    let ws = Workspace::new("empty");
    ws.write(
        "nyth.toml",
        "[modules.git]\nsource = \"./dotfiles/gitconfig\"\ntarget = \".gitconfig\"\n",
    );
    ws.write("dotfiles/gitconfig", "[user]\nname = untouched");
    fs::create_dir_all(ws.paths().upper).expect("create empty upper dir");

    let report = commit_into(&ws.config_path(), &ws.paths()).expect("commit should succeed");

    assert!(report.applied.is_empty());
    let untouched = fs::read_to_string(ws.root.join("dotfiles/gitconfig")).expect("read repo file");
    assert_eq!(untouched, "[user]\nname = untouched");
}
