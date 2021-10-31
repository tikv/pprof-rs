// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::HashMap;
use std::fmt::{Debug, Formatter};

use parking_lot::RwLock;

use crate::frames::{Frames, UnresolvedFrames};
use crate::profiler::Profiler;

use crate::{Error, Result};

/// The final presentation of a report which is actually an `HashMap` from `Frames` to isize (count).
pub struct Report {
    /// key is a backtrace captured by profiler and value is count of it.
    pub data: HashMap<Frames, isize>,
    pub sample_rate: libc::c_int,
    pub start: std::time::SystemTime,
    pub until: std::time::SystemTime,
}

/// The presentation of an unsymbolicated report which is actually an `HashMap` from `UnresolvedFrames` to isize (count).
pub struct UnresolvedReport {
    /// key is a backtrace captured by profiler and value is count of it.
    pub data: HashMap<UnresolvedFrames, isize>,
}

/// A builder of `Report` and `UnresolvedReport`. It builds report from a running `Profiler`.
pub struct ReportBuilder<'a> {
    frames_post_processor: Option<Box<dyn Fn(&mut Frames)>>,
    profiler: &'a RwLock<Result<Profiler>>,

    sample_rate: libc::c_int,
    start: std::time::SystemTime,
    until: std::time::SystemTime,
}

unsafe impl Send for ReportBuilder<'_> {}
unsafe impl Sync for ReportBuilder<'_> {}
unsafe impl Send for Report {}
unsafe impl Sync for Report {}

impl<'a> ReportBuilder<'a> {
    pub(crate) fn new(
        profiler: &'a RwLock<Result<Profiler>>,
        sample_rate: libc::c_int,
        start: std::time::SystemTime,
        until: std::time::SystemTime,
    ) -> Self {
        Self {
            frames_post_processor: None,
            profiler,

            sample_rate,
            start,
            until,
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

    /// Build an `UnresolvedReport`
    pub fn build_unresolved(&self) -> Result<UnresolvedReport> {
        let mut hash_map = HashMap::new();

        match self.profiler.read().as_ref() {
            Err(err) => {
                log::error!("Error in creating profiler: {}", err);
                Err(Error::CreatingError)
            }
            Ok(profiler) => {
                profiler.data.try_iter()?.for_each(|entry| {
                    let count = entry.count;
                    if count > 0 {
                        let key = &entry.item;
                        match hash_map.get_mut(key) {
                            Some(value) => {
                                *value += count;
                            }
                            None => {
                                match hash_map.insert(key.clone(), count) {
                                    None => {}
                                    Some(_) => {
                                        unreachable!();
                                    }
                                };
                            }
                        }
                    }
                });

                Ok(UnresolvedReport { data: hash_map })
            }
        }
    }

    /// Build a `Report`.
    pub fn build(&self) -> Result<Report> {
        let mut hash_map = HashMap::new();

        match self.profiler.write().as_mut() {
            Err(err) => {
                log::error!("Error in creating profiler: {}", err);
                Err(Error::CreatingError)
            }
            Ok(profiler) => {
                profiler.data.try_iter()?.for_each(|entry| {
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

                Ok(Report {
                    data: hash_map,
                    sample_rate: self.sample_rate,
                    start: self.start,
                    until: self.until,
                })
            }
        }
    }
}

/// This will generate Report in a human-readable format:
///
/// ```shell
/// FRAME: pprof::profiler::perf_signal_handler::h7b995c4ab2e66493 -> FRAME: Unknown -> FRAME: {func1} ->
/// FRAME: {func2} -> FRAME: {func3} ->  THREAD: {thread_name} {count}
/// ```
impl Debug for Report {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        for (key, val) in self.data.iter() {
            write!(f, "{:?} {}", key, val)?;
            writeln!(f)?;
        }

        Ok(())
    }
}

impl Report {
    pub fn fold<W>(&self, with_thread_name: bool, mut writer: W) -> Result<()>
    where
        W: std::io::Write,
    {
        for (key, value) in self.data.iter() {
            if with_thread_name {
                if !key.thread_name.is_empty() {
                    write!(writer, "{};", key.thread_name)?;
                } else {
                    write!(writer, "{:?};", key.thread_id)?;
                }
            }

            let last_frame = key.frames.len() - 1;
            for (index, frame) in key.frames.iter().rev().enumerate() {
                let last_symbol = frame.len() - 1;
                for (index, symbol) in frame.iter().rev().enumerate() {
                    if index == last_symbol {
                        write!(writer, "{}", symbol)?;
                    } else {
                        write!(writer, "{};", symbol)?;
                    }
                }

                if index != last_frame {
                    write!(writer, ";")?;
                }
            }

            writeln!(writer, " {}", value)?;
        }

        Ok(())
    }
}

#[cfg(feature = "pyroscope")]
mod pyroscope {
    use super::*;

    impl Report {
        pub async fn pyroscope_ingest<S: AsRef<str>, N: AsRef<str>>(
            &self,
            url: S,
            application_name: N,
        ) -> Result<()> {
            let mut buffer = Vec::new();

            self.fold(true, &mut buffer)?;

            let client = reqwest::Client::new();
            // TODO: handle the error of this request
            client
                .post(format!("{}/ingest", url.as_ref()))
                .header("Content-Type", "application/json")
                .query(&[
                    ("name", application_name.as_ref()),
                    (
                        "from",
                        &format!(
                            "{}",
                            self.start
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                        ),
                    ),
                    (
                        "until",
                        &format!(
                            "{}",
                            self.until
                                .duration_since(std::time::UNIX_EPOCH)
                                .unwrap()
                                .as_secs()
                        ),
                    ),
                    ("format", "folded"),
                    ("sampleRate", &format!("{}", self.sample_rate)),
                    ("spyName", "pprof-rs"),
                ])
                .body(buffer)
                .send()
                .await?;

            Ok(())
        }
    }
}

#[cfg(feature = "flamegraph")]
mod flamegraph {
    use super::*;
    use inferno::flamegraph;
    use std::io::Write;

    impl Report {
        /// `flamegraph` will write an svg flamegraph into `writer` **only available with `flamegraph` feature**
        pub fn flamegraph<W>(&self, writer: W) -> Result<()>
        where
            W: Write,
        {
            self.flamegraph_with_options(writer, &mut flamegraph::Options::default())
        }

        /// same as `flamegraph`, but accepts custom `options` for the flamegraph
        pub fn flamegraph_with_options<W>(
            &self,
            writer: W,
            options: &mut flamegraph::Options,
        ) -> Result<()>
        where
            W: Write,
        {
            if !self.data.is_empty() {
                let mut buffer = Vec::new();

                self.fold(true, &mut buffer)?;
                flamegraph::from_reader::<&[u8], _>(options, buffer.as_ref(), writer).unwrap();
                // TODO: handle this error
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
                        dudup_str.insert(symbol.sys_name().into_owned());
                        dudup_str.insert(symbol.filename().into_owned());
                    }
                }
            }
            // string table's first element must be an empty string
            let mut str_tbl = vec!["".to_owned()];
            str_tbl.extend(dudup_str.into_iter());

            let mut strings = HashMap::new();
            for (index, name) in str_tbl.iter().enumerate() {
                strings.insert(name.as_str(), index);
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
                        let function_id = fn_tbl.len() as u64 + 1;
                        let function = protos::Function {
                            id: function_id,
                            name: *strings.get(name.as_str()).unwrap() as i64,
                            system_name: *strings.get(sys_name.as_ref()).unwrap() as i64,
                            filename: *strings.get(filename.as_ref()).unwrap() as i64,
                            ..protos::Function::default()
                        };
                        functions.insert(name, function_id);
                        let line = protos::Line {
                            function_id,
                            line: lineno as i64,
                        };
                        let loc = protos::Location {
                            id: function_id,
                            line: vec![line],
                            ..protos::Location::default()
                        };
                        // the fn_tbl has the same length with loc_tbl
                        fn_tbl.push(function);
                        loc_tbl.push(loc);
                        // current frame locations
                        locs.push(function_id);
                    }
                }
                let sample = protos::Sample {
                    location_id: locs,
                    value: vec![*count as i64],
                    ..protos::Sample::default()
                };
                samples.push(sample);
            }
            let (type_idx, unit_idx) = (str_tbl.len(), str_tbl.len() + 1);
            str_tbl.push("cpu".to_owned());
            str_tbl.push("count".to_owned());
            let sample_type = protos::ValueType {
                r#type: type_idx as i64,
                unit: unit_idx as i64,
            };
            let profile = protos::Profile {
                sample_type: vec![sample_type],
                sample: samples,
                string_table: str_tbl,
                function: fn_tbl,
                location: loc_tbl,
                ..protos::Profile::default()
            };
            Ok(profile)
        }
    }
}
