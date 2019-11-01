use crate::frames::Frames;
use crate::profiler::Profiler;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::{Error, Result};

pub struct Report {
    pub data: HashMap<Frames, usize>,
}

pub struct ReportBuilder<'a> {
    frames_post_processor: Option<Box<dyn Fn(&mut Frames)>>,
    profiler: &'a spin::RwLock<Result<Profiler>>,
}

impl<'a> ReportBuilder<'a> {
    pub fn new(profiler: &'a spin::RwLock<Result<Profiler>>) -> Self {
        Self {
            frames_post_processor: None,
            profiler,
        }
    }

    pub fn frames_post_processor<T>(&mut self, frames_post_processor: T) -> &mut Self
    where
        T: Fn(&mut Frames) + 'static,
    {
        self.frames_post_processor
            .replace(Box::new(frames_post_processor));

        self
    }

    pub fn build(&mut self) -> Result<Report> {
        let mut hash_map = HashMap::new();

        match self.profiler.write().as_mut() {
            Err(err) => {
                log::error!("Error in creating profiler: {}", err);
                Err(Error::CreatingError)
            }
            Ok(profiler) => {
                profiler.data.iter()?.for_each(|entry| {
                    let count = entry.count;
                    if count > 0 {
                        let mut key = Frames::from(entry.item.clone());
                        if let Some(processor) = &self.frames_post_processor {
                            processor(&mut key);
                        }

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

                Ok(Report { data: hash_map })
            }
        }
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
