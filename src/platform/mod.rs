#[cfg(any(target_os = "linux", target_os = "macos"))]
mod nix_impl {
    pub mod addr_validate;
    pub mod profiler;
    pub mod timer;
}

#[cfg(target_os = "windows")]
mod windows_impl {
    pub mod addr_validate;
    pub mod profiler;
    pub mod timer;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use nix_impl::*;

#[cfg(target_os = "windows")]
pub use windows_impl::*;
