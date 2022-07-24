#[cfg(any(target_os = "linux", target_os = "macos"))]
mod nix_impl {
    pub mod addr_validate;
    pub mod profiler;
    pub mod timer;

    #[cfg(all(
        any(target_arch = "x86_64", target_arch = "aarch64"),
        feature = "frame-pointer",
    ))]
    mod frame_pointer;
    #[cfg(all(
        any(target_arch = "x86_64", target_arch = "aarch64"),
        feature = "frame-pointer",
    ))]
    pub use frame_pointer::Trace as TraceImpl;

    #[cfg(not(all(
        any(target_arch = "x86_64", target_arch = "aarch64"),
        feature = "frame-pointer",
    )))]
    #[path = "../backtrace_rs.rs"]
    mod backtrace_rs;
    #[cfg(not(all(
        any(target_arch = "x86_64", target_arch = "aarch64"),
        feature = "frame-pointer",
    )))]
    pub use backtrace_rs::Trace as TraceImpl;
}

#[cfg(target_os = "windows")]
mod windows_impl {
    pub mod addr_validate;
    pub mod profiler;
    pub mod timer;

    #[cfg(feature = "frame-pointer")]
    std::compile_error!("frame-pointer feature is currently not supported on windows.");

    #[path = "../backtrace_rs.rs"]
    mod backtrace_rs;
    pub use backtrace_rs::Trace as TraceImpl;
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub use nix_impl::*;

#[cfg(target_os = "windows")]
pub use windows_impl::*;
