use backtrace::Frame;
use rustc_demangle::demangle;
use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;
use std::fmt::{Display, Error as FmtError, Formatter};
use std::hash::{Hash, Hasher};
use std::os::raw::c_void;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct UnresolvedFrames {
    pub(crate) frames: Vec<Frame>,
}

impl From<Vec<Frame>> for UnresolvedFrames {
    fn from(bt: Vec<Frame>) -> Self {
        Self {
            frames: bt[2..].to_vec(),
        }
    }
}

impl PartialEq for UnresolvedFrames {
    fn eq(&self, other: &Self) -> bool {
        if self.frames.len() == other.frames.len() {
            let iter = self.frames.iter().zip(other.frames.iter());

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
        self.frames
            .iter()
            .for_each(|frame| frame.symbol_address().hash(state))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Symbol {
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
            None => match &other.name {
                Some(_) => false,
                None => true,
            },
        }
    }
}

impl Serialize for Symbol {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut symbol = serializer.serialize_struct("Symbol", 4)?;
        symbol.serialize_field("name", &self.name)?;
        symbol.serialize_field(
            "addr",
            &match self.addr {
                Some(addr) => Some(addr as u64),
                None => None,
            },
        )?;
        symbol.serialize_field("lineno", &self.lineno)?;
        symbol.serialize_field("filename", &self.filename)?;
        symbol.end()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Frames {
    pub(crate) frames: Vec<Vec<Symbol>>,
}

impl From<UnresolvedFrames> for Frames {
    fn from(frames: UnresolvedFrames) -> Self {
        let mut fs = Vec::new();
        frames.frames.iter().for_each(|frame| {
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
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        for frame in self.frames.iter() {
            write!(f, "FRAME: ")?;
            for symbol in frame.iter() {
                write!(f, "{} -> ", symbol)?;
            }
        }

        Ok(())
    }
}
