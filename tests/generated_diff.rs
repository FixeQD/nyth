mod support;

use std::path::PathBuf;

use nyth::cli::generated_diff::{
    diff_generated_config, read_generated_change, render_generated_change, GeneratedChange,
    LineDiff,
};
use support::Workspace;

#[test]
fn identical_content_produces_no_diff_lines() {
    let content = "[user]\nname = test\n";
    assert_eq!(diff_generated_config(content, content), vec![]);
}

#[test]
fn changed_value_on_one_line_pairs_into_a_single_changed_entry() {
    let before = "[user]\n    name = Jan K\n";
    let after = "[user]\n    name = Jan Kowalski\n";

    let diff = diff_generated_config(before, after);

    assert_eq!(
        diff,
        vec![LineDiff::Changed {
            before: "    name = Jan K".to_string(),
            after: "    name = Jan Kowalski".to_string(),
        }]
    );
}

#[test]
fn a_newly_added_line_with_no_matching_removal_is_just_added() {
    let before = "[user]\n    name = Jan Kowalski\n";
    let after = "[user]\n    name = Jan Kowalski\n[alias]\n    st = status -sb\n";

    let diff = diff_generated_config(before, after);

    assert_eq!(
        diff,
        vec![
            LineDiff::Added("[alias]".to_string()),
            LineDiff::Added("    st = status -sb".to_string()),
        ]
    );
}

#[test]
fn a_removed_line_with_no_replacement_is_just_removed() {
    let before = "[user]\n    name = Jan Kowalski\n[alias]\n    st = status -sb\n";
    let after = "[user]\n    name = Jan Kowalski\n";

    let diff = diff_generated_config(before, after);

    assert_eq!(
        diff,
        vec![
            LineDiff::Removed("[alias]".to_string()),
            LineDiff::Removed("    st = status -sb".to_string()),
        ]
    );
}

#[test]
fn empty_before_content_treats_every_line_as_added() {
    let diff = diff_generated_config("", "one\ntwo\n");
    assert_eq!(
        diff,
        vec![
            LineDiff::Added("one".to_string()),
            LineDiff::Added("two".to_string()),
        ]
    );
}

#[test]
fn read_generated_change_reads_before_from_home_and_after_from_upper() {
    let ws = Workspace::new("generated-diff");
    ws.write(".gitconfig", "[user]\n    name = Jan K\n");
    ws.write(
        "state/upper/.gitconfig",
        "[user]\n    name = Jan Kowalski\n",
    );

    let change = read_generated_change(&ws.root, &ws.paths().upper, &PathBuf::from(".gitconfig"))
        .expect("both files exist, this should succeed");

    assert_eq!(
        change,
        GeneratedChange {
            relative_path: PathBuf::from(".gitconfig"),
            lines: vec![LineDiff::Changed {
                before: "    name = Jan K".to_string(),
                after: "    name = Jan Kowalski".to_string(),
            }],
        }
    );
}

#[test]
fn read_generated_change_treats_a_missing_before_file_as_empty_not_an_error() {
    let ws = Workspace::new("generated-diff-missing-before");
    ws.write("state/upper/.newly-generated.conf", "fresh = true\n");

    let change = read_generated_change(
        &ws.root,
        &ws.paths().upper,
        &PathBuf::from(".newly-generated.conf"),
    )
    .expect("a missing 'before' file is a valid all-added diff, not a read error");

    assert_eq!(
        change.lines,
        vec![LineDiff::Added("fresh = true".to_string())]
    );
}

#[test]
fn render_never_claims_to_know_the_nix_option_and_frames_changes_plainly() {
    let change = GeneratedChange {
        relative_path: PathBuf::from(".gitconfig"),
        lines: vec![
            LineDiff::Changed {
                before: "name = Jan K".to_string(),
                after: "name = Jan Kowalski".to_string(),
            },
            LineDiff::Added("st = status -sb".to_string()),
        ],
    };

    let rendered = render_generated_change(&change);

    assert!(rendered.contains("programs.*"));
    assert!(rendered.contains("was: name = Jan K"));
    assert!(rendered.contains("now: name = Jan Kowalski"));
    assert!(rendered.contains("added:\n    st = status -sb"));
    assert!(rendered.contains("doesn't know which Nix option"));
}
