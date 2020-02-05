// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use backtrace::Frame;
use rustc_demangle::demangle;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::os::raw::c_void;
use std::path::PathBuf;

use crate::{MAX_DEPTH, MAX_THREAD_NAME};

#[derive(Debug, Clone)]
pub struct UnresolvedFramesSlice<'a> {
    pub frames: &'a [Frame],
    pub thread_name: &'a [u8],
    pub thread_id: u64,
}

pub struct UnresolvedFrames {
    pub frames: [Frame; MAX_DEPTH],
    pub depth: usize,
    pub thread_name: [u8; MAX_THREAD_NAME],
    pub thread_name_length: usize,
    pub thread_id: u64,
}

impl Clone for UnresolvedFrames {
    fn clone(&self) -> Self {
        let slice = self.slice().clone();
        Self::new(slice.frames, slice.thread_name, slice.thread_id)
    }
}

impl Debug for UnresolvedFrames {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        self.slice().fmt(f)
    }
}

impl UnresolvedFrames {
    pub fn new(bt: &[Frame], tn: &[u8], thread_id: u64) -> Self {
        let depth = bt.len();
        let mut frames: [Frame; MAX_DEPTH] =
            unsafe { std::mem::MaybeUninit::uninit().assume_init() };
        frames[0..depth].clone_from_slice(bt);

        let thread_name_length = tn.len();
        let mut thread_name: [u8; MAX_THREAD_NAME] =
            unsafe { std::mem::MaybeUninit::uninit().assume_init() };
        thread_name[0..thread_name_length].clone_from_slice(tn);

        Self {
            frames,
            depth,
            thread_name,
            thread_name_length,
            thread_id,
        }
    }

    fn slice(&self) -> UnresolvedFramesSlice {
        UnresolvedFramesSlice {
            frames: &self.frames[0..self.depth],
            thread_name: &self.thread_name[0..self.thread_name_length],
            thread_id: self.thread_id,
        }
    }
}

impl PartialEq for UnresolvedFrames {
    fn eq(&self, other: &Self) -> bool {
        if self.thread_id == other.thread_id {
            if self.depth == other.depth {
                let iter = self.slice().frames.iter().zip(other.slice().frames.iter());

                iter.map(|(self_frame, other_frame)| {
                    self_frame.symbol_address() == other_frame.symbol_address()
                })
                .all(|result| result)
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl Eq for UnresolvedFrames {}

impl Hash for UnresolvedFrames {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.slice()
            .frames
            .iter()
            .for_each(|frame| frame.symbol_address().hash(state));
        self.thread_id.hash(state);
    }
}

/// Symbol is a representation of a function symbol. It contains name and addr of it. If built with
/// debug message, it can also provide line number and filename. The name in it is not demangled.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// This name is raw name of a symbol (which hasn't been demangled).
    pub name: Option<Vec<u8>>,

    /// The address of the function. It is not 100% trustworthy.
    pub addr: Option<*mut c_void>,

    /// Line number of this symbol. If compiled with debug message, you can get it.
    pub lineno: Option<u32>,

    /// Filename of this symbol. If compiled with debug message, you can get it.
    pub filename: Option<PathBuf>,
}

impl Symbol {
    pub fn name(&self) -> String {
        match &self.name {
            Some(name) => match std::str::from_utf8(&name) {
                Ok(name) => format!("{}", demangle(name)),
                Err(_) => "NonUtf8Name".to_owned(),
            },
            None => "Unknown".to_owned(),
        }
    }

    pub fn sys_name(&self) -> &str {
        match &self.name {
            Some(name) => match std::str::from_utf8(&name) {
                Ok(name) => name,
                Err(_) => "NonUtf8Name",
            },
            None => "Unknown",
        }
    }

    pub fn filename(&self) -> &str {
        match &self.filename {
            Some(name) => match name.as_os_str().to_str() {
                Some(name) => name,
                None => "NonUtf8Name",
            },
            None => "Unknown",
        }
    }

    pub fn lineno(&self) -> u32 {
        self.lineno.unwrap_or(0)
    }
}

unsafe impl Send for Symbol {}

impl From<&backtrace::Symbol> for Symbol {
    fn from(symbol: &backtrace::Symbol) -> Self {
        Symbol {
            name: symbol.name().map(|name| name.as_bytes().to_vec()),
            addr: symbol.addr(),
            lineno: symbol.lineno(),
            filename: symbol.filename().map(|filename| filename.to_owned()),
        }
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut Formatter) -> Result<(), std::fmt::Error> {
        match &self.name {
            Some(name) => match std::str::from_utf8(&name) {
                Ok(name) => write!(f, "{}", demangle(name))?,
                Err(_) => write!(f, "NonUtf8Name")?,
            },
            None => {
                write!(f, "Unknown")?;
            }
        }
        Ok(())
    }
}

impl PartialEq for Symbol {
    fn eq(&self, other: &Self) -> bool {
        match &self.name {
            Some(name) => match &other.name {
                Some(other_name) => name == other_name,
                None => false,
            },
            None => other.name.is_none(),
        }
    }
}

/// A representation of a backtrace. `thread_name` and `thread_id` was got from `pthread_getname_np`
/// and `pthread_self`. frames is a vector of symbols.
#[derive(Debug, Clone)]
pub struct Frames {
    pub frames: Vec<Vec<Symbol>>,
    pub thread_name: String,
    pub thread_id: u64,
}

impl From<UnresolvedFrames> for Frames {
    fn from(frames: UnresolvedFrames) -> Self {
        let mut fs = Vec::new();

        // These variables are used to filter out signal handler functions
        // We should find a more robust way to do this. On way is to extend
        // backtrace-rs to get signal handler information from it.
        let after_signal_handler = &mut -1;
        let is_signal_handler = &mut false;

        frames.slice().frames.iter().for_each(|frame| {
            let mut symbols = Vec::new();

            backtrace::resolve_frame(frame, |symbol| {
                let symbol = Symbol::from(symbol);
                if &symbol.name() == "perf_signal_handler" {
                    *is_signal_handler = true;
                }

                symbols.push(symbol);
            });

            if !symbols.is_empty() && *after_signal_handler > 0 {
                fs.push(symbols);
            }

            if *is_signal_handler {
                *after_signal_handler += 1;
            }
        });

        Self {
            frames: fs,
            thread_name: unsafe {
                String::from_utf8_unchecked(
                    frames.thread_name[0..frames.thread_name_length].to_vec(),
                )
            },
            thread_id: frames.thread_id,
        }
    }
}

impl PartialEq for Frames {
    fn eq(&self, other: &Self) -> bool {
        if self.thread_name == other.thread_name {
            if self.frames.len() == other.frames.len() {
                let iter = self.frames.iter().zip(other.frames.iter());

                iter.map(|(self_frame, other_frame)| {
                    if self_frame.len() == other_frame.len() {
                        let iter = self_frame.iter().zip(other_frame.iter());
                        iter.map(|(self_symbol, other_symbol)| self_symbol == other_symbol)
                            .all(|result| result)
                    } else {
                        false
                    }
                })
                .all(|result| result)
            } else {
                false
            }
        } else {
            false
        }
    }
}

impl Eq for Frames {}

impl Hash for Frames {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.frames.iter().for_each(|frame| {
            frame.iter().for_each(|symbol| match &symbol.name {
                Some(name) => name.hash(state),
                None => 0.hash(state),
            })
        });
        self.thread_name.hash(state);
    }
}

impl Display for Frames {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        for frame in self.frames.iter() {
            write!(f, "FRAME: ")?;
            for symbol in frame.iter() {
                write!(f, "{} -> ", symbol)?;
            }
        }
        write!(f, "THREAD: ")?;
        if !self.thread_name.is_empty() {
            write!(f, "{}", self.thread_name)?;
        } else {
            write!(f, "ThreadId({})", self.thread_id)?;
        }

        Ok(())
    }
}
