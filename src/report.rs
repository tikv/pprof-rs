use crate::frames::{Frames, UnresolvedFrames};
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::Result;

pub struct Report {
    data: HashMap<Frames, usize>,
}

impl Report {
    pub(crate) fn from_collector(data: &mut Collector<UnresolvedFrames>) -> Result<Self> {
        let mut hash_map = HashMap::new();

        data.iter()?.for_each(|entry| {
            let count = entry.count;
            if count > 0 {
                let key = Frames::from(entry.item.clone());

                match hash_map.get_mut(&key) {
                    Some(value) => {
                        *value += count;
                    }
                    None => {
                        match hash_map.insert(key, count) {
                            None => {}
                            Some(_) => {
                                unreachable!();
                            }
                        };
                    }
                }
            }
        });

        Ok(Self { data: hash_map })
    }
}

impl Display for Report {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        for (key, val) in self.data.iter() {
            write!(f, "{} {}", key, val)?;
            writeln!(f)?;
        }

        Ok(())
    }
}

use crate::collector::Collector;
#[cfg(feature = "flamegraph")]
use std::io::Write;

#[cfg(feature = "flamegraph")]
impl Report {
    pub fn flamegraph<W>(&self, writer: W) -> Result<()>
    where
        W: Write,
    {
        use inferno::flamegraph;

        let lines: Vec<String> = self
            .data
            .iter()
            .map(|(key, value)| {
                let mut line = String::new();
                if key.thread_name.len() > 0 {
                    line.push_str(&key.thread_name);
                } else {
                    line.push_str(&format!("{:?}", key.thread_id));
                }
                line.push(';');

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
