#[cfg(any(target_os = "linux", target_os = "macos"))]
mod platform_nix {
    pub mod addr_validate;
    pub mod profiler;
    pub mod timer;
}

#[cfg(target_os = "windows")]
mod platform_windows {
    pub mod addr_validate;
    pub mod profiler;
    pub mod timer;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use platform_nix::*;

#[cfg(target_os = "windows")]
pub use platform_windows::*;
