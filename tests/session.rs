use std::fs;
use std::process::exit;

use nyth::build::build_into;
use nyth::error::{NamespaceError, NythError};
use nyth::session::run_session_with;
use nyth::sys::namespace::CallerIdentity;
use nyth::sys::paths::NythPaths;

#[test]
fn run_session_execs_target_with_modules_and_home_snapshot_in_place() {
    let real_identity = CallerIdentity::from_current_process().expect("syscalls don't fail");

    match unsafe { libc::fork() } {
        -1 => panic!("fork failed"),
        0 => exit(run_in_child(&real_identity)),
        child_pid => {
            let mut status = 0;
            unsafe { libc::waitpid(child_pid, &mut status, 0) };
            assert!(libc::WIFEXITED(status), "child did not exit normally");
            assert_eq!(
                libc::WEXITSTATUS(status),
                0,
                "see child stderr above for which step failed"
            );
        }
    }
}

fn run_in_child(real_identity: &CallerIdentity) -> i32 {
    let workspace = std::env::temp_dir().join(format!("nyth-session-test-{}", std::process::id()));
    let _ = fs::remove_dir_all(&workspace);

    let fake_home = workspace.join("home");
    if fs::create_dir_all(&fake_home).is_err() {
        eprintln!("create fake home failed");
        return 1;
    }

    // Already in $HOME before any module is applied, to prove home-snapshot keeps fr the rest of $HOME visible, not just the overlaid modules
    if fs::write(fake_home.join("preexisting.txt"), b"was already here").is_err() {
        eprintln!("seed fake home failed");
        return 2;
    }

    let dotfiles = workspace.join("dotfiles");
    if fs::create_dir_all(&dotfiles).is_err()
        || fs::write(dotfiles.join("gitconfig"), "[user]\nname = nyth-test").is_err()
    {
        eprintln!("seed module source failed");
        return 3;
    }

    let config_path = workspace.join("nyth.toml");
    let config = "[modules.git]\nsource = \"./dotfiles/gitconfig\"\ntarget = \".gitconfig\"\n";
    if fs::write(&config_path, config).is_err() {
        eprintln!("write config failed");
        return 4;
    }

    let identity = CallerIdentity {
        uid: real_identity.uid,
        gid: real_identity.gid,
        home: fake_home.clone(),
    };
    let paths = NythPaths {
        root: workspace.join("state"),
        lower: workspace.join("state/lower"),
        upper: workspace.join("state/upper"),
        work: workspace.join("state/work"),
    };

    if let Err(e) = build_into(&config_path, &paths.lower) {
        eprintln!("build_into failed: {e:?}");
        return 5;
    }

    // Proves: module content visible, pre-existing $HOME content still visible
    let marker = workspace.join("session-ran.txt");
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

    let error = run_session_with(&config_path, &target_command, &identity, &paths);

    if let NythError::Namespace(NamespaceError::UserNamespacesDisabled) = error {
        // Same as tests/namespace.rs and tests/overlay.rs: environment-dependent
        return 0;
    }

    eprintln!("run_session_with returned without a successful exec: {error}");
    99
}
