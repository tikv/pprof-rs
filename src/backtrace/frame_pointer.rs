// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use std::ptr::null_mut;

use libc::{backtrace, c_void};

#[derive(Clone, Debug)]
pub struct Frame {
    pub ip: usize,
}

extern "C" {
    fn _Unwind_FindEnclosingFunction(pc: *mut c_void) -> *mut c_void;

}

impl super::Frame for Frame {
    type S = backtrace::Symbol;

    fn resolve_symbol<F: FnMut(&Self::S)>(&self, cb: F) {
        backtrace::resolve(self.ip as *mut c_void, cb);
    }

    fn symbol_address(&self) -> *mut libc::c_void {
        if cfg!(target_os = "macos") || cfg!(target_os = "ios") {
            self.ip as *mut c_void
        } else {
            unsafe { _Unwind_FindEnclosingFunction(self.ip as *mut c_void) }
        }
    }
}

pub fn trace<F: FnMut(&Frame) -> bool>(mut cb: F) {
    let mut backtraces: [*mut *mut c_void; 32] = [null_mut(); 32];

    let length = unsafe {
        let ret = backtrace(backtraces.as_mut_ptr() as *mut *mut c_void, backtraces.len() as i32);
        if ret < 0 {
            return;
        } else {
            ret as usize
        }
    };

    for backtrace in backtraces[0..length].iter() {
        let frame = Frame {
            ip: *backtrace as usize,
        };
        if !cb(&frame) {
            break;
        }
    }
}

pub use backtrace::Symbol;