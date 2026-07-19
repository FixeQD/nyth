pub mod identity;
pub mod overlay;
pub mod paths;

pub(crate) fn errno() -> i32 {
    unsafe { *libc::__errno_location() }
}
