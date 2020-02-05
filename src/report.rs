// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use crate::frames::Frames;
use crate::profiler::Profiler;
use std::collections::HashMap;
use std::fmt::{Display, Formatter};

use crate::{Error, Result};

/// The final presentation of a report which is actually an `HashMap` from `Frames` to usize (count).
pub struct Report {
    /// key is a backtrace captured by profiler and value is count of it.
    pub data: HashMap<Frames, isize>,
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
mod flamegraph {
    use super::*;
    use std::io::Write;

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
                    if !key.thread_name.is_empty() {
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
            if !lines.is_empty() {
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
}

#[cfg(feature = "protobuf")]
mod protobuf {
    use super::*;
    use crate::protos;
    use std::collections::HashSet;

    impl Report {
        // `pprof` will generate google's pprof format report
        pub fn pprof(&self) -> crate::Result<protos::Profile> {
            let mut dudup_str = HashSet::new();
            for key in self.data.iter().map(|(key, _)| key) {
                for frame in key.frames.iter() {
                    for symbol in frame {
                        dudup_str.insert(symbol.name());
                        dudup_str.insert(symbol.sys_name().to_owned());
                        dudup_str.insert(symbol.filename().to_owned());
                    }
                }
            }
            // string table's first element must be an empty string
            let mut str_tbl = vec!["".to_owned()];
            str_tbl.extend(dudup_str.into_iter());

            let mut strings = HashMap::new();
            for (index, name) in str_tbl.iter().enumerate() {
                strings.insert(name, index);
            }

            let mut samples = vec![];
            let mut loc_tbl = vec![];
            let mut fn_tbl = vec![];
            let mut functions = HashMap::new();
            for (key, count) in self.data.iter() {
                let mut locs = vec![];
                for frame in key.frames.iter() {
                    for symbol in frame {
                        let name = symbol.name();
                        if let Some(loc_idx) = functions.get(&name) {
                            locs.push(*loc_idx);
                            continue;
                        }
                        let sys_name = symbol.sys_name();
                        let filename = symbol.filename();
                        let lineno = symbol.lineno();
                        let mut function = protos::Function::default();
                        let id = fn_tbl.len() as u64 + 1;
                        function.id = id;
                        function.name = *strings.get(&name).unwrap() as i64;
                        function.system_name = *strings.get(&sys_name.to_owned()).unwrap() as i64;
                        function.filename = *strings.get(&filename.to_owned()).unwrap() as i64;
                        functions.insert(name, id);
                        let mut line = protos::Line::default();
                        line.function_id = id;
                        line.line = lineno as i64;
                        let mut loc = protos::Location::default();
                        loc.id = id;
                        loc.line = vec![line];
                        // the fn_tbl has the same length with loc_tbl
                        fn_tbl.push(function);
                        loc_tbl.push(loc);
                        // current frame locations
                        locs.push(id);
                    }
                }
                let mut sample = protos::Sample::default();
                sample.location_id = locs;
                sample.value = vec![*count as i64];
                samples.push(sample);
            }
            let (type_idx, unit_idx) = (str_tbl.len(), str_tbl.len() + 1);
            str_tbl.push("cpu".to_owned());
            str_tbl.push("count".to_owned());
            let mut sample_type = protos::ValueType::default();
            sample_type.r#type = type_idx as i64;
            sample_type.unit = unit_idx as i64;
            let mut profile = protos::Profile::default();
            profile.sample_type = vec![sample_type];
            profile.sample = samples;
            profile.string_table = str_tbl;
            profile.function = fn_tbl;
            profile.location = loc_tbl;
            Ok(profile)
        }
    }
}
