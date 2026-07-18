use std::fs;
use std::path::PathBuf;

use nyth::sys::paths::NythPaths;

/// A throwaway directory under /tmp, torn down when it goes out of scope
/// (even on panic, unlike a manual `remove_dir_all` at the end of a test).
/// For tests that need plain file I/O (commit/status) with no mounting or
/// namespaces involved.
pub struct Workspace {
    pub root: PathBuf,
}

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

    /// A full NythPaths layout rooted in this workspace, for tests
    /// exercising `commit_into`/`nyth_status`, which need upper/work too.
    pub fn paths(&self) -> NythPaths {
        NythPaths {
            lower: self.root.join("state/lower"),
            upper: self.root.join("state/upper"),
            work: self.root.join("state/work"),
            root: self.root.join("state"),
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
/// Every test that calls a real `unshare()` needs this: `unshare(CLONE_NEWUSER)`
/// returns EINVAL if the calling process is multithreaded, which it always is
/// under cargo test's own harness, regardless of `--test-threads`. A freshly
/// forked child is always single-threaded no matter how busy the parent was,
/// so it's the only way to exercise `enter_isolated_session` at all here.
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
