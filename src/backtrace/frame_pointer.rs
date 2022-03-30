// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use std::ptr::null_mut;

use libc::c_void;

#[derive(Clone, Debug)]
pub struct Frame {
    pub ip: usize,
}

extern "C" {
    fn _Unwind_FindEnclosingFunction(pc: *mut c_void) -> *mut c_void;

}

impl super::Frame for Frame {
    type S = backtrace::Symbol;

    fn ip(&self) -> usize {
        self.ip
    }

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

pub struct Trace {}
impl super::Trace for Trace {
    type Frame = Frame;

    fn trace<F: FnMut(&Self::Frame) -> bool>(ucontext: *mut libc::c_void, mut cb: F) {
        let ucontext: *mut libc::ucontext_t = ucontext as *mut libc::ucontext_t;
        
        #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
        let frame_pointer = unsafe { (*ucontext).uc_mcontext.gregs[libc::REG_RBP as usize] as usize };

        #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
        let frame_pointer = unsafe {
            let mcontext = (*ucontext).uc_mcontext;
            if mcontext.is_null() {
                0
            } else {
                (*mcontext).__ss.__rbp as usize
            }
        };

        // TODO: support arm64

        let mut frame_pointer = frame_pointer as *mut FramePointerLayout;
        let mut last_frame_pointer = null_mut();
        loop {
            // The stack grow from high address to low address.
            // for glibc, we have a `__libc_stack_end` to get the highest address of stack.
            #[cfg(target_env = "gnu")]
            if frame_pointer > __libc_stack_end {
                break;
            }

            // the frame pointer should never be smaller than the former one.
            if frame_pointer < last_frame_pointer {
                break;
            }
            last_frame_pointer = frame_pointer;

            // iterate to the next frame
            let frame = Frame {
                ip: unsafe { (*frame_pointer).ret },
            };

            if !cb(&frame) {
                break;
            }
            frame_pointer = unsafe { (*frame_pointer).frame_pointer };
        }
    }
}

extern "C" {
    static __libc_stack_end: *mut FramePointerLayout;
}

#[cfg(target_arch = "x86_64")]
#[repr(C)]
struct FramePointerLayout {
    frame_pointer: *mut FramePointerLayout,
    ret: usize,
}


pub use backtrace::Symbol;
