impl super::Frame for backtrace::Frame {
    type S = backtrace::Symbol;

    fn resolve_symbol<F: FnMut(&Self::S)>(&self, cb: F) {
        backtrace::resolve_frame(self, cb);
    }

    fn symbol_address(&self) -> *mut libc::c_void {
        self.symbol_address()
    }
}

pub fn trace<F: FnMut(&Frame) -> bool>(cb: F) {
    unsafe { backtrace::trace_unsynchronized(cb) }
}

pub use backtrace::Frame;
pub use backtrace::Symbol;
