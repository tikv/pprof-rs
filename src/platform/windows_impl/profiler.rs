use crate::error::Result;
use crate::profiler::{Profiler, ProfilerImpl};

impl ProfilerImpl for Profiler {
    fn register(&mut self) -> Result<()> {
        unimplemented!()
    }
    fn unregister(&mut self) -> Result<()> {
        unimplemented!()
    }
}
