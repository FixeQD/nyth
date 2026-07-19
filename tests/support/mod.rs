use std::fs;
use std::path::PathBuf;

use nyth::sys::paths::NythPaths;

/// A throwaway directory under /tmp, torn down when it goes out of scope
/// (even on panic, unlike a manual `remove_dir_all` at the end of a test).
/// For tests that need plain file I/O (commit/status) with no mounting involved
#[allow(dead_code)]
pub struct Workspace {
    pub root: PathBuf,
}

#[allow(dead_code)]
impl Workspace {
    /// `name` only has to be unique within one test *binary* (commit.rs,
    /// status.rs, etc. are separate processes, so reusing a name across
    /// binaries is fine); it has to be unique among tests that run
    /// concurrently in the same binary, since cargo test runs `#[test]`s in
    /// parallel threads of the same process.
    pub fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!("nyth-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create workspace root");
        Self { root }
    }

    pub fn write(&self, relative: &str, contents: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).expect("create parent dirs");
        fs::write(path, contents).expect("write workspace file");
    }

    /// A full `NythPaths` layout rooted in this workspace, for tests exercising `commit_into`/`nyth_status`, which need `upper`/`work` too
    pub fn paths(&self) -> NythPaths {
        let root = self.root.join("state");
        NythPaths {
            lower: root.join("lower"),
            home_snapshot: root.join("home-snapshot"),
            upper: root.join("upper"),
            work: root.join("work"),
            root,
        }
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// Forks, runs `child_fn` in the child (must return an exit code, 0 meaning
/// success), and asserts the child exited with code 0.
///
/// Used by tests that need a fresh, single-threaded process to safely call
/// root-only syscalls (mount/unmount/chown) without disturbing the rest of
/// the test harness.
///
/// `child_fn` never returns to the caller on the success path: the child
/// exits from inside this function, not back in the test.
pub fn run_in_fork(child_fn: impl FnOnce() -> i32) {
    match unsafe { libc::fork() } {
        -1 => panic!("fork failed"),
        0 => std::process::exit(child_fn()),
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
