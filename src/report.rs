use crate::frames::{Frames, UnresolvedFrames};
use serde::Serialize;
use std::collections::HashMap;
use std::fmt::{Display, Error as FmtError, Formatter};

#[derive(Serialize, Debug)]
pub struct Report {
    data: HashMap<Frames, i32>,
}

impl From<&HashMap<UnresolvedFrames, i32>> for Report {
    fn from(data: &HashMap<UnresolvedFrames, i32>) -> Self {
        let data = data
            .iter()
            .map(|(key, value)| (Frames::from(key.clone()), *value))
            .collect();
        Self { data }
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> {
        for (key, val) in self.data.iter() {
            write!(f, "{} {}", key, val)?;
            writeln!(f)?;
        }

        Ok(())
    }
}

#[cfg(feature = "flamegraph")]
use std::io::Write;

#[cfg(feature = "flamegraph")]
impl Report {
    pub fn flamegraph<W>(&self, writer: W) -> crate::Result<()>
    where
        W: Write,
    {
        use inferno::flamegraph;

        let lines: Vec<String> = self
            .data
            .iter()
            .map(|(key, value)| {
                let mut line = String::new();

                for frame in key.frames.iter().rev() {
                    for symbol in frame.iter().rev() {
                        line.push_str(&format!("{}/", symbol));
                    }
                    line.pop().unwrap_or_default();
                    line.push(';');
                }

                line.pop().unwrap_or_default();
                line.push_str(&format!(" {}", value));

                line
            })
            .collect();
        if lines.len() > 0 {
            flamegraph::from_lines(
                &mut flamegraph::Options::default(),
                lines.iter().map(|s| &**s),
                writer,
            )
            .unwrap(); // TODO: handle this error
        }

        Ok(())
    }
}
