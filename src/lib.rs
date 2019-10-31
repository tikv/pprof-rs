#[macro_use]
extern crate quick_error;

pub const MAX_DEPTH: usize = 31;

mod error;
pub mod frames;
mod profiler;
mod report;
pub mod collector;
pub mod timer;

pub use error::*;
pub use profiler::{ProfilerGuard, PROFILER};
pub use report::*;
