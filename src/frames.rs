use backtrace::Frame;
use rustc_demangle::demangle;
use std::fmt::{Debug, Display, Formatter};
use std::hash::{Hash, Hasher};
use std::os::raw::c_void;
use std::path::PathBuf;

use crate::MAX_DEPTH;

#[derive(Debug, Clone)]
pub struct UnresolvedFramesSlice<'a> {
    pub frames: &'a [Frame],
}

pub struct UnresolvedFrames {
    pub frames: [Frame; MAX_DEPTH],
    pub depth: usize,
}

impl Clone for UnresolvedFrames {
    fn clone(&self) -> Self {
        Self::new(self.slice().clone().frames)
    }
}

impl Debug for UnresolvedFrames {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        self.slice().fmt(f)
    }
}

impl UnresolvedFrames {
    pub fn new(bt: &[Frame]) -> Self {
        let depth = bt.len();
        let mut frames: [Frame; MAX_DEPTH] =
            unsafe { std::mem::MaybeUninit::uninit().assume_init() };
        frames[0..depth].clone_from_slice(bt);
        Self { frames, depth }
    }

    fn slice(&self) -> UnresolvedFramesSlice {
        UnresolvedFramesSlice {
            frames: &self.frames[0..self.depth],
        }
    }
}

impl PartialEq for UnresolvedFrames {
    fn eq(&self, other: &Self) -> bool {
        if self.frames.len() == other.frames.len() {
            let iter = self.frames[0..self.depth].iter().zip(other.frames.iter());

            iter.map(|(self_frame, other_frame)| {
                self_frame.symbol_address() == other_frame.symbol_address()
            })
            .all(|result| result)
        } else {
            false
        }
    }
}

impl Eq for UnresolvedFrames {}

impl Hash for UnresolvedFrames {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.frames[0..self.depth]
            .iter()
            .for_each(|frame| frame.symbol_address().hash(state))
    }
}

#[derive(Debug, Clone)]
pub struct Symbol {
    name: Option<Vec<u8>>,
    addr: Option<*mut c_void>,
    lineno: Option<u32>,
    filename: Option<PathBuf>,
}

unsafe impl Send for Symbol {}

impl From<&backtrace::Symbol> for Symbol {
    fn from(symbol: &backtrace::Symbol) -> Self {
        Symbol {
            name: symbol
                .name()
                .and_then(|name| Some(name.as_bytes().to_vec())),
            addr: symbol.addr(),
            lineno: symbol.lineno(),
            filename: symbol
                .filename()
                .and_then(|filename| Some(filename.to_owned())),
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

#[derive(Debug, Clone)]
pub struct Frames {
    pub frames: Vec<Vec<Symbol>>,
}

impl From<UnresolvedFrames> for Frames {
    fn from(frames: UnresolvedFrames) -> Self {
        let mut fs = Vec::new();
        frames.frames[0..frames.depth].iter().for_each(|frame| {
            let mut symbols = Vec::new();
            backtrace::resolve_frame(frame, |symbol| {
                symbols.push(Symbol::from(symbol));
            });
            fs.push(symbols);
        });

        Self { frames: fs }
    }
}

impl PartialEq for Frames {
    fn eq(&self, other: &Self) -> bool {
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
        })
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

        Ok(())
    }
}
