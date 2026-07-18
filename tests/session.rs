mod support;

use std::fs;
use std::os::unix::fs::symlink;

use nyth::cli::session::run_session_with;
use nyth::config::RelativeHomePath;
use nyth::error::{NamespaceError, NythError};
use nyth::sys::namespace::CallerIdentity;
use support::Workspace;

#[test]
fn run_session_execs_target_with_watched_paths_and_home_snapshot_in_place() {
    let real_identity = CallerIdentity::from_current_process().expect("syscalls don't fail");
    support::run_in_fork(|| run_in_child(&real_identity));
}

fn run_in_child(real_identity: &CallerIdentity) -> i32 {
    let ws = Workspace::new("session");
    let fake_home = ws.root.join("home");
    if fs::create_dir_all(&fake_home).is_err() {
        eprintln!("create fake home failed");
        return 1;
    }

    // Already in $HOME before any watched path is resolved, to prove home-snapshot keeps the rest of $HOME visible, not just the watched entries
    ws.write("home/preexisting.txt", "was already here");

    // Stands in for what Home Manager actually leaves in $HOME
    ws.write("store/gitconfig", "[user]\nname = nyth-test");
    let store_target = ws.root.join("store/gitconfig");
    if symlink(&store_target, fake_home.join(".gitconfig")).is_err() {
        eprintln!("failed to create symlink the way home-manager would");
        return 2;
    }

    let identity = CallerIdentity {
        uid: real_identity.uid,
        gid: real_identity.gid,
        home: fake_home.clone(),
    };
    let paths = ws.paths();
    let watched = match RelativeHomePath::new(".gitconfig") {
        Ok(path) => path,
        Err(e) => {
            eprintln!("RelativeHomePath::new failed: {e}");
            return 3;
        }
    };

    // Proves: watched-path content visible, pre-existing $HOME content still visible
    let marker = ws.root.join("session-ran.txt");
    let target_command = vec![
        "/bin/sh".to_string(),
        "-c".to_string(),
        format!(
            "test -f {}/.gitconfig && test -f {}/preexisting.txt && touch {}",
            fake_home.display(),
            fake_home.display(),
            marker.display()
        ),
    ];

    let error = run_session_with(
        std::slice::from_ref(&watched),
        &[],
        &target_command,
        &identity,
        &paths,
    );

    if let NythError::Namespace(NamespaceError::UserNamespacesDisabled) = error {
        // Same as tests/namespace.rs and tests/overlay.rs: environment-dependent
        return 0;
    }

    eprintln!("run_session_with returned without a successful exec: {error}");
    99
}
