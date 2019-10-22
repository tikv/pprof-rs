use backtrace::{BacktraceFrame, Backtrace};
use std::hash::{Hash, Hasher};
use std::fmt::{Display, Formatter, Error as FmtError};

#[derive(Debug)]
pub(crate) struct Frames {
    pub(crate) frames: Vec<BacktraceFrame>
}

impl From<Backtrace> for Frames {
    fn from(bt: Backtrace) -> Self {
        Self {
            frames: bt.frames().to_vec(),
        }
    }
}

impl PartialEq for Frames {
    fn eq(&self, other: &Self) -> bool {
        if self.frames.len() == other.frames.len() {
            let iter = self.frames.iter().zip(other.frames.iter());

            iter.map(|(self_frame, other_frame)| {
                if self_frame.symbols().len() == other_frame.symbols().len() {
                    let iter = self_frame.symbols().iter().zip(other_frame.symbols().iter());
                    iter.map(|(self_symbol, other_symbol)| {
                        self_symbol.addr() == other_symbol.addr()
                    }).all(|result| result)
                } else {
                    false
                }
            }).all(|result| result)
        } else {
            false
        }
    }
}

impl Eq for Frames {

}

impl Hash for Frames {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.frames.iter().for_each(|frame| {
            frame.symbols().iter().for_each(|symbol| {
                symbol.addr().hash(state);
            })
        })
    }
}

impl Display for Frames {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        for frame in self.frames.iter() {
            write!(f, "FRAME: ");
            for symbol in frame.symbols().iter() {
                match symbol.name() {
                    Some(name) => {
                        match name.as_str() {
                            Some(name) => {
                                write!(f, "{}:", name);
                            }
                            None => {
                                write!(f, "NotValidUtf8:");
                            }
                        }
//                        write!(f, "{} -> ", name);
                    }
                    None => {
                        write!(f, "Unknown:");
                    }
                }

                match symbol.addr() {
                    Some(addr) => {
                        write!(f, "{:?} -> ", addr);
                    }
                    None => {
                        write!(f, "Unknown -> ");
                    }
                }
            }
        }

        Ok(())
    }
}