use std::fs;
use std::process::exit;

use nyth::error::NamespaceError;
use nyth::sys::namespace::{CallerIdentity, enter_isolated_session};
use nyth::sys::overlay::{mount_overlay, provision_scratch_tmpfs};

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
