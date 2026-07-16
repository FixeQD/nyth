mod support;

use std::fs;

use nyth::cli::build::build_into;
use nyth::cli::session::run_session_with;
use nyth::error::{NamespaceError, NythError};
use nyth::sys::namespace::CallerIdentity;
use support::Workspace;

#[test]
fn run_session_execs_target_with_modules_and_home_snapshot_in_place() {
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

    // Already in $HOME before any module is applied, to prove home-snapshot keeps fr the rest of $HOME visible, not just the overlaid modules
    ws.write("home/preexisting.txt", "was already here");
    ws.write("dotfiles/gitconfig", "[user]\nname = nyth-test");
    ws.write(
        "nyth.toml",
        "[modules.git]\nsource = \"./dotfiles/gitconfig\"\ntarget = \".gitconfig\"\n",
    );

    let identity = CallerIdentity {
        uid: real_identity.uid,
        gid: real_identity.gid,
        home: fake_home.clone(),
    };
    let paths = ws.paths();

    if let Err(e) = build_into(&ws.config_path(), &paths.lower) {
        eprintln!("build_into failed: {e:?}");
        return 5;
    }

    // Proves: module content visible, pre-existing $HOME content still visible
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

    let error = run_session_with(&ws.config_path(), &target_command, &identity, &paths);

    if let NythError::Namespace(NamespaceError::UserNamespacesDisabled) = error {
        // Same as tests/namespace.rs and tests/overlay.rs: environment-dependent
        return 0;
    }

    eprintln!("run_session_with returned without a successful exec: {error}");
    99
}
