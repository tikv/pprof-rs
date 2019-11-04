use crate::frames::Frames;
use crate::profiler::Profiler;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::{Error, Result};

/// The final presentation of a report which is actually an `HashMap` from `Frames` to usize (count).
pub struct Report {
    /// key is a backtrace captured by profiler and value is count of it.
    pub data: HashMap<Frames, usize>,
}

/// A builder of `Report`. It builds report from a running `Profiler`.
pub struct ReportBuilder<'a> {
    frames_post_processor: Option<Box<dyn Fn(&mut Frames)>>,
    profiler: &'a spin::RwLock<Result<Profiler>>,
}

impl<'a> ReportBuilder<'a> {
    pub(crate) fn new(profiler: &'a spin::RwLock<Result<Profiler>>) -> Self {
        Self {
            frames_post_processor: None,
            profiler,
        }
    }

    /// Set `frames_post_processor` of a `ReportBuilder`. Before finally building a report, `frames_post_processor`
    /// will be applied to every Frames.
    pub fn frames_post_processor<T>(&mut self, frames_post_processor: T) -> &mut Self
    where
        T: Fn(&mut Frames) + 'static,
    {
        self.frames_post_processor
            .replace(Box::new(frames_post_processor));

        self
    }

    /// Build a `Report`.
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

/// This will print Report in a human-readable format:
///
/// ```shell
/// FRAME: pprof::profiler::perf_signal_handler::h7b995c4ab2e66493 -> FRAME: Unknown -> FRAME: {func1} ->
/// FRAME: {func2} -> FRAME: {func3} ->  THREAD: {thread_name} {count}
/// ```
///
/// This format is **not** stable! Never try to parse it and get profile. `data` field in `Report` is
/// public for read and write. You can do anything you want with it.
///
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
    /// `flamegraph` will write an svg flamegraph into `writer` **only available with `flamegraph` feature**
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
