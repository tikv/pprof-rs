// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    NixError(#[from] nix::Error),
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
