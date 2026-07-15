pub mod namespace;
pub mod overlay;

pub(crate) fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}
