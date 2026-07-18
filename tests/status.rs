mod support;

use std::path::PathBuf;

use nyth::cli::status::{
    DotfilesRepo, PendingChange, UpperEntry, diff_upper_against_repo, nyth_status,
};
use nyth::config::RelativeHomePath;
use support::Workspace;

fn watched(target: &str) -> RelativeHomePath {
    RelativeHomePath::new(target).expect("valid relative path")
}

#[test]
fn diff_marks_watched_paths_as_watched_path_modified() {
    let repo = DotfilesRepo::new(PathBuf::from("/unused"), vec![watched(".gitconfig")]);
    let entries = vec![UpperEntry {
        relative_path: PathBuf::from(".gitconfig"),
    }];

    let changes = diff_upper_against_repo(&entries, &repo);

    assert_eq!(
        changes,
        vec![PendingChange::WatchedPathModified {
            relative_path: PathBuf::from(".gitconfig"),
        }]
    );
}

// A file changed inside a directory watched-path still counts as watched, even though its path isn't literally equal to the watched-path itself
#[test]
fn diff_marks_nested_file_under_directory_watched_path_as_watched_path_modified() {
    let repo = DotfilesRepo::new(PathBuf::from("/unused"), vec![watched(".config/hypr")]);
    let entries = vec![UpperEntry {
        relative_path: PathBuf::from(".config/hypr/hyprland.conf"),
    }];

    let changes = diff_upper_against_repo(&entries, &repo);

    assert_eq!(
        changes,
        vec![PendingChange::WatchedPathModified {
            relative_path: PathBuf::from(".config/hypr/hyprland.conf"),
        }]
    );
}

#[test]
fn diff_marks_unwatched_paths_as_untracked() {
    let repo = DotfilesRepo::new(PathBuf::from("/unused"), vec![watched(".gitconfig")]);
    let entries = vec![UpperEntry {
        relative_path: PathBuf::from(".config/random-app/state.db"),
    }];

    let changes = diff_upper_against_repo(&entries, &repo);

    assert_eq!(
        changes,
        vec![PendingChange::Untracked {
            relative_path: PathBuf::from(".config/random-app/state.db"),
        }]
    );
}

#[test]
fn diff_is_pure_no_watched_paths_no_entries_no_changes() {
    let repo = DotfilesRepo::new(PathBuf::from("/unused"), vec![]);
    assert_eq!(diff_upper_against_repo(&[], &repo), vec![]);
}

#[test]
fn nyth_status_walks_upper_recursively_and_diffs_against_repo() {
    let ws = Workspace::new("status");
    let paths = ws.paths();

    ws.write(
        "state/upper/.config/hypr/hyprland.conf",
        "monitor=,preferred,auto,1",
    );
    ws.write("state/upper/.gitconfig", "[user]\nname = test");
    ws.write("state/upper/random-state.db", "");

    let repo = DotfilesRepo::new(
        ws.root.join("dotfiles"),
        vec![watched(".gitconfig"), watched(".config/hypr")],
    );

    let mut changes = nyth_status(&paths, &repo).expect("status should succeed");
    changes.sort_by_key(|c| match c {
        PendingChange::WatchedPathModified { relative_path } => relative_path.clone(),
        PendingChange::Untracked { relative_path } => relative_path.clone(),
    });

    assert_eq!(
        changes,
        vec![
            PendingChange::WatchedPathModified {
                relative_path: PathBuf::from(".config/hypr/hyprland.conf"),
            },
            PendingChange::WatchedPathModified {
                relative_path: PathBuf::from(".gitconfig"),
            },
            PendingChange::Untracked {
                relative_path: PathBuf::from("random-state.db"),
            },
        ]
    );
}
