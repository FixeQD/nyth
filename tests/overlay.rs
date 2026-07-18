use std::fs;
use std::os::unix::fs::symlink;
use std::process::exit;

use nyth::config::RelativeHomePath;
use nyth::error::NamespaceError;
use nyth::sys::namespace::{CallerIdentity, enter_isolated_session};
use nyth::sys::overlay::{
    ScratchTmpfs, mount_overlay, provision_scratch_tmpfs, resolve_watched_paths,
};

#[test]
fn mount_overlay_merges_lower_and_allows_writes() {
    let identity = CallerIdentity::from_current_process().expect("syscalls don't fail");

    match unsafe { libc::fork() } {
        -1 => panic!("fork failed"),
        0 => exit(run_in_child(&identity)),
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
fn run_in_child(identity: &CallerIdentity) -> i32 {
    match enter_isolated_session(identity.uid, identity.gid) {
        Ok(_) => {}
        Err(NamespaceError::UserNamespacesDisabled) => return 0,
        Err(e) => {
            eprintln!("enter_isolated_session failed: {e:?}");
            return 1;
        }
    }

    let scratch = match provision_scratch_tmpfs(identity) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("provision_scratch_tmpfs failed: {e:?}");
            return 2;
        }
    };

    // Not the real NythPaths::lower (that's identity.home-scoped and persistent), just a throwaway dir for this test
    let target = scratch.root.join("target");
    let lower = scratch.root.join("test-lower");
    let upper = scratch.root.join("test-upper");
    let work = scratch.root.join("test-work");
    for dir in [&target, &lower, &upper, &work] {
        if fs::create_dir(dir).is_err() {
            eprintln!("create test dir failed: {}", dir.display());
            return 3;
        }
    }

    if let Err(e) = fs::write(lower.join("testfile"), b"from-lower") {
        eprintln!("seed lowerdir failed: {e}");
        return 4;
    }

    if let Err(e) = mount_overlay(&lower, &upper, &work, &scratch, &target) {
        eprintln!("mount_overlay failed: {e:?}");
        return 5;
    }

    let seen = match fs::read(target.join("testfile")) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("reading through overlay failed: {e}");
            return 6;
        }
    };
    if seen != b"from-lower" {
        eprintln!("lowerdir content did not surface through overlay");
        return 7;
    }

    if let Err(e) = fs::write(target.join("newfile"), b"from-session") {
        eprintln!("write through overlay failed: {e}");
        return 8;
    }

    match fs::read(upper.join("newfile")) {
        Ok(bytes) if bytes == b"from-session" => 0,
        Ok(_) => {
            eprintln!("upperdir file had unexpected content");
            9
        }
        Err(e) => {
            eprintln!("write did not land in upperdir: {e}");
            10
        }
    }
}

#[test]
fn resolve_watched_paths_binds_dereferenced_store_target_not_the_symlink() {
    let identity = CallerIdentity::from_current_process().expect("syscalls don't fail");

    match unsafe { libc::fork() } {
        -1 => panic!("fork failed"),
        0 => exit(run_resolve_in_child(&identity)),
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

fn run_resolve_in_child(identity: &CallerIdentity) -> i32 {
    match enter_isolated_session(identity.uid, identity.gid) {
        Ok(_) => {}
        Err(NamespaceError::UserNamespacesDisabled) => return 0,
        Err(e) => {
            eprintln!("enter_isolated_session failed: {e:?}");
            return 1;
        }
    }

    let root = std::env::temp_dir().join(format!("nyth-watched-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    let home_snapshot = root.join("home-snapshot");
    let lower = root.join("lower");
    // Stands in for a real /nix/store output: a plain, read-only file HM's symlink points at
    let fake_store_target = root.join("fake-store-gitconfig");

    if fs::create_dir_all(&home_snapshot).is_err() || fs::create_dir_all(&lower).is_err() {
        eprintln!("failed to set up test dirs");
        return 2;
    }
    if fs::write(&fake_store_target, b"from-nix-store").is_err() {
        eprintln!("failed to seed fake store file");
        return 3;
    }
    // What Home Manager actually leaves in $HOME: an absolute symlink into the store
    if symlink(&fake_store_target, home_snapshot.join(".gitconfig")).is_err() {
        eprintln!("failed to create symlink the way home-manager would");
        return 4;
    }

    let scratch = ScratchTmpfs {
        root: root.clone(),
        home_snapshot,
    };
    let watched = match RelativeHomePath::new(".gitconfig") {
        Ok(path) => path,
        Err(e) => {
            eprintln!("RelativeHomePath::new failed: {e}");
            return 5;
        }
    };

    if let Err(e) = resolve_watched_paths(&scratch, &lower, std::slice::from_ref(&watched)) {
        eprintln!("resolve_watched_paths failed: {e:?}");
        return 6;
    }

    let lower_entry = lower.join(".gitconfig");
    let metadata = match fs::symlink_metadata(&lower_entry) {
        Ok(metadata) => metadata,
        Err(e) => {
            eprintln!("stat on lower entry failed: {e}");
            return 7;
        }
    };
    if metadata.file_type().is_symlink() {
        eprintln!("lower/.gitconfig is still a symlink - write-through would escape the overlay");
        return 8;
    }

    match fs::read(&lower_entry) {
        Ok(bytes) if bytes == b"from-nix-store" => 0,
        Ok(_) => {
            eprintln!("lower/.gitconfig content did not match the dereferenced store target");
            9
        }
        Err(e) => {
            eprintln!("reading through the resolved bind mount failed: {e}");
            10
        }
    }
}
