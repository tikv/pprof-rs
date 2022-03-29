// Copyright 2022 TiKV Project Authors. Licensed under Apache-2.0.

use libc::c_void;
use std::path::PathBuf;

pub trait Symbol: Sized {
    fn name(&self) -> Option<Vec<u8>>;
    fn addr(&self) -> Option<*mut c_void>;
    fn lineno(&self) -> Option<u32>;
    fn filename(&self) -> Option<PathBuf>;
}

impl Symbol for backtrace::Symbol {
    fn name(&self) -> Option<Vec<u8>> {
        self.name().map(|name| name.as_bytes().to_vec())
    }

    fn addr(&self) -> Option<*mut libc::c_void> {
        self.addr()
    }

    fn lineno(&self) -> Option<u32> {
        self.lineno()
    }

    fn filename(&self) -> Option<std::path::PathBuf> {
        self.filename().map(|filename| filename.to_owned())
    }
}

pub trait Frame: Sized + Clone {
    type S: Symbol;

    fn resolve_symbol<F: FnMut(&Self::S)>(&self, cb: F);
    fn symbol_address(&self) -> *mut c_void;
}

// #[cfg(feature = "backtrace-rs")]
// mod backtrace_rs;
// #[cfg(feature = "backtrace-rs")]
// pub use backtrace_rs::{trace, Frame as FrameImpl, Symbol as SymbolImpl};

#[cfg(feature = "frame-pointer")]
mod frame_pointer;
#[cfg(feature = "frame-pointer")]
pub use frame_pointer::{trace, Frame as FrameImpl, Symbol as SymbolImpl};