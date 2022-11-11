// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use std::ptr::null_mut;

use libc::c_void;

use crate::addr_validate::validate;

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
        if ucontext.is_null() {
            return;
        }

        #[cfg(all(target_arch = "x86_64", target_os = "linux"))]
        let frame_pointer =
            unsafe { (*ucontext).uc_mcontext.gregs[libc::REG_RBP as usize] as usize };

        #[cfg(all(target_arch = "x86_64", target_os = "macos"))]
        let frame_pointer = unsafe {
            let mcontext = (*ucontext).uc_mcontext;
            if mcontext.is_null() {
                0
            } else {
                (*mcontext).__ss.__rbp as usize
            }
        };

        #[cfg(all(target_arch = "aarch64", target_os = "linux"))]
        let frame_pointer = unsafe { (*ucontext).uc_mcontext.regs[29] as usize };

        #[cfg(all(target_arch = "aarch64", target_os = "macos"))]
        let frame_pointer = unsafe {
            let mcontext = (*ucontext).uc_mcontext;
            if mcontext.is_null() {
                0
            } else {
                (*mcontext).__ss.__fp as usize
            }
        };

        #[cfg(all(target_arch = "riscv64", target_os = "linux"))]
        let frame_pointer = unsafe { (*ucontext).uc_mcontext.__gregs[libc::REG_S0] as usize };

        #[cfg(all(target_arch = "loongarch64", target_os = "linux"))]
        let frame_pointer = unsafe { (*ucontext).uc_mcontext.sc_regs[22] as usize };

        let mut frame_pointer = frame_pointer as *mut FramePointerLayout;

        let mut last_frame_pointer: *mut FramePointerLayout = null_mut();
        loop {
            // The stack grow from high address to low address.
            // but we don't have a reasonable assumption for the hightest address
            // the `__libc_stack_end` is not thread-local, and only represent the
            // stack end of the main thread. For other thread, their stacks are allocated
            // by the `pthread`.
            //
            // TODO: If we can hook the thread creation, we will have chance to get the
            // stack end through `pthread_get_attr`.

            // the frame pointer should never be smaller than the former one.
            if !last_frame_pointer.is_null() && frame_pointer < last_frame_pointer {
                break;
            }

            if !validate(frame_pointer as *const libc::c_void) {
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

#[repr(C)]
struct FramePointerLayout {
    frame_pointer: *mut FramePointerLayout,
    ret: usize,
}
