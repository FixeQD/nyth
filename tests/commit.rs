mod support;

use std::fs;

use nyth::cli::commit::commit_into;
use support::Workspace;

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
