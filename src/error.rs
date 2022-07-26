// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

// TODO Windows error is not finished
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[cfg(target_os = "windows")]
    #[error("{0}")]
    OsError(i32),

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[error("{0}")]
    OsError(#[from] nix::Error),

    #[error("{0}")]
    IoError(#[from] std::io::Error),
    #[error("create profiler error")]
    CreatingError,
    #[error("start running cpu profiler error")]
    Running,
    #[error("stop running cpu profiler error")]
    NotRunning,
}

pub type Result<T> = std::result::Result<T, Error>;
