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
                        for symbol in frame.symbols.iter().rev() {
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

#[cfg(any(feature = "prost-protobuf", feature = "rust-protobuf"))]
mod protobuf {
    use super::*;
    use pprof_protobuf as protos;

    struct LookupTable<T: std::cmp::Eq + std::hash::Hash, E> {
        pub map: HashMap<T, usize>,
        pub table: Vec<E>,
    }

    impl<T, E> LookupTable<T, E>
    where
        T: std::cmp::Eq + std::hash::Hash,
    {
        pub fn new() -> Self {
            Self {
                map: HashMap::new(),
                table: Vec::new(),
            }
        }

        pub fn lookup_or_insert(&mut self, key: T, content: E) -> (usize, &mut E) {
            let idx = if let Some(idx) = self.map.get(&key) {
                *idx
            } else {
                let idx = self.table.len();
                self.table.push(content);
                self.map.insert(key, idx);

                idx
            };

            let ret = &mut self.table[idx];

            (idx, ret)
        }
    }

    impl Report {
        // `pprof` will generate google's pprof format report
        pub fn pprof(&self) -> crate::Result<protos::ProfileProtobuf> {
            let mut string_table = LookupTable::new();
            // string table's first element must be an empty string
            string_table.lookup_or_insert("".to_owned(), "".to_owned());

            let mut location_table = LookupTable::new();
            let mut function_table = LookupTable::new();

            let mut samples = vec![];

            for (key, count) in self.data.iter() {
                let mut locations = vec![];

                for frame in key.frames.iter() {
                    let mut location = protos::Location::default();
                    location.set_address(frame.ip);
                    let mut lines = vec![];

                    for symbol in frame.symbols.iter() {
                        let name = symbol.name();
                        let sys_name = symbol.sys_name();
                        let filename = symbol.filename();
                        let lineno = symbol.lineno();

                        let mut line = protos::Line::default();
                        line.set_line(lineno as i64);

                        let mut function = protos::Function::default();

                        function.set_name(
                            string_table.lookup_or_insert(name.clone(), name.clone()).0 as i64,
                        );
                        function.set_system_name(
                            string_table
                                .lookup_or_insert(sys_name.to_owned(), sys_name.to_owned())
                                .0 as i64,
                        );
                        function.set_filename(
                            string_table
                                .lookup_or_insert(filename.to_owned(), filename.to_owned())
                                .0 as i64,
                        );
                        function.set_start_line(lineno as i64); // TODO: get start line of function in backtrace-rs

                        let (idx, function) =
                            function_table.lookup_or_insert(function.get_name(), function.clone());
                        function.set_id(idx as u64 + 1);
                        line.set_function_id(function.get_id());

                        lines.push(line);
                    }
                    location.set_line(lines);

                    let (idx, location) =
                        location_table.lookup_or_insert(location.get_address(), location.clone());

                    location.set_id(idx as u64 + 1);
                    locations.push(location.get_id());
                }

                let mut sample = protos::Sample::default();
                sample.set_location_id(locations);
                sample.set_value(vec![*count as i64]);

                let mut labels = vec![];
                let mut label = protos::Label::default();
                label.set_key(
                    string_table
                        .lookup_or_insert("thread".to_owned(), "thread".to_owned())
                        .0 as i64,
                );
                label.set_str(
                    string_table
                        .lookup_or_insert(key.thread_name.clone(), key.thread_name.clone())
                        .0 as i64,
                );
                labels.push(label);

                sample.set_label(labels);
                samples.push(sample);
            }

            let mut sample_type = protos::ValueType::default();
            sample_type.set_type(
                string_table
                    .lookup_or_insert("cpu".to_owned(), "cpu".to_owned())
                    .0 as i64,
            );
            sample_type.set_unit(
                string_table
                    .lookup_or_insert("count".to_owned(), "count".to_owned())
                    .0 as i64,
            );

            let mut profile = protos::Profile::default();
            profile.set_sample_type(vec![sample_type]);
            profile.set_sample(samples);
            profile.set_string_table(string_table.table);
            profile.set_function(function_table.table);
            profile.set_location(location_table.table);
            Ok(profile.into())
        }
    }
}
