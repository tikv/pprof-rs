#[macro_use]
extern crate quick_error;

pub const MAX_DEPTH: usize = 31;

mod error;
mod frames;
mod profiler;
mod report;
mod collector;
mod timer;

pub use error::*;
pub use profiler::{ProfilerGuard, PROFILER};
pub use report::*;
