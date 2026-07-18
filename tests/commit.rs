mod support;

use std::fs;
use std::path::PathBuf;

use nyth::cli::commit::commit_into;
use nyth::cli::status::DotfilesRepo;
use nyth::config::{Module, RelativeHomePath};
use support::Workspace;

fn module(source: &str, target: &str) -> Module {
    Module {
        source: PathBuf::from(source),
        target: RelativeHomePath::new(target).expect("valid relative path"),
        on_change: None,
    }
}

#[test]
fn commit_writes_module_modified_file_back_to_repo_source() {
    let ws = Workspace::new("basic");
    ws.write("dotfiles/gitconfig", "[user]\nname = old");
    let repo = DotfilesRepo::new(
        ws.root.join("dotfiles"),
        vec![("git".to_string(), module("gitconfig", ".gitconfig"))],
    );
    let paths = ws.paths();
    ws.write("state/upper/.gitconfig", "[user]\nname = new");

    let report = commit_into(&repo, &paths).expect("commit should succeed");

    assert_eq!(report.applied, vec![ws.root.join("dotfiles/gitconfig")]);
    let written = fs::read_to_string(ws.root.join("dotfiles/gitconfig")).expect("read repo file");
    assert_eq!(written, "[user]\nname = new");
}

#[test]
fn commit_ignores_untracked_files() {
    let ws = Workspace::new("untracked");
    let repo = DotfilesRepo::new(ws.root.join("dotfiles"), vec![]);
    let paths = ws.paths();
    ws.write("state/upper/random-state.db", "");

    let report = commit_into(&repo, &paths).expect("commit should succeed");

    assert!(report.applied.is_empty());
}

#[test]
fn commit_writes_nested_directory_module_file_to_right_subpath() {
    let ws = Workspace::new("nested");
    ws.write("dotfiles/hypr/hyprland.conf", "monitor=old");
    let repo = DotfilesRepo::new(
        ws.root.join("dotfiles"),
        vec![("hyprland".to_string(), module("hypr", ".config/hypr"))],
    );
    let paths = ws.paths();
    ws.write("state/upper/.config/hypr/hyprland.conf", "monitor=new");

    let report = commit_into(&repo, &paths).expect("commit should succeed");

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
    ws.write("dotfiles/gitconfig", "[user]\nname = untouched");
    let repo = DotfilesRepo::new(
        ws.root.join("dotfiles"),
        vec![("git".to_string(), module("gitconfig", ".gitconfig"))],
    );
    fs::create_dir_all(ws.paths().upper).expect("create empty upper dir");

    let report = commit_into(&repo, &ws.paths()).expect("commit should succeed");

    assert!(report.applied.is_empty());
    let untouched = fs::read_to_string(ws.root.join("dotfiles/gitconfig")).expect("read repo file");
    assert_eq!(untouched, "[user]\nname = untouched");
}
