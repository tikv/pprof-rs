// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::collections::HashMap;
use std::fmt::{Debug, Formatter};

use spin::RwLock;

use crate::frames::{Frames, UnresolvedFrames};
use crate::profiler::Profiler;
use crate::timer::ReportTiming;

use crate::{Error, Result};

/// The final presentation of a report which is actually an `HashMap` from `Frames` to isize (count).
pub struct Report {
    /// Key is a backtrace captured by profiler and value is count of it.
    pub data: HashMap<Frames, isize>,

    /// Collection frequency, start time, duration.
    pub timing: ReportTiming,
}

/// The presentation of an unsymbolicated report which is actually an `HashMap` from `UnresolvedFrames` to isize (count).
pub struct UnresolvedReport {
    /// key is a backtrace captured by profiler and value is count of it.
    pub data: HashMap<UnresolvedFrames, isize>,

    /// Collection frequency, start time, duration.
    pub timing: ReportTiming,
}

type FramesPostProcessor = Box<dyn Fn(&mut Frames)>;

/// A builder of `Report` and `UnresolvedReport`. It builds report from a running `Profiler`.
pub struct ReportBuilder<'a> {
    frames_post_processor: Option<FramesPostProcessor>,
    profiler: &'a RwLock<Result<Profiler>>,
    timing: ReportTiming,
}

impl<'a> ReportBuilder<'a> {
    pub(crate) fn new(profiler: &'a RwLock<Result<Profiler>>, timing: ReportTiming) -> Self {
        Self {
            frames_post_processor: None,
            profiler,
            timing,
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

                Ok(UnresolvedReport {
                    data: hash_map,
                    timing: self.timing.clone(),
                })
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
                    timing: self.timing.clone(),
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

#[cfg(feature = "flamegraph")]
mod flamegraph {
    use super::*;
    use inferno::flamegraph;
    use std::fmt::Write;

    impl Report {
        /// `flamegraph` will write an svg flamegraph into `writer` **only available with `flamegraph` feature**
        pub fn flamegraph<W>(&self, writer: W) -> Result<()>
        where
            W: std::io::Write,
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
            W: std::io::Write,
        {
            let lines: Vec<String> = self
                .data
                .iter()
                .map(|(key, value)| {
                    let mut line = key.thread_name_or_id();
                    line.push(';');

                    for frame in key.frames.iter().rev() {
                        for symbol in frame.iter().rev() {
                            write!(&mut line, "{};", symbol).unwrap();
                        }
                    }

                    line.pop().unwrap_or_default();
                    write!(&mut line, " {}", value).unwrap();

                    line
                })
                .collect();
            if !lines.is_empty() {
                flamegraph::from_lines(options, lines.iter().map(|s| &**s), writer).unwrap();
                // TODO: handle this error
            }

            Ok(())
        }
    }
}

#[cfg(feature = "_protobuf")]
#[allow(clippy::useless_conversion)]
#[allow(clippy::needless_update)]
mod protobuf {
    use super::*;
    use crate::protos;
    use std::collections::HashSet;
    use std::time::SystemTime;

    const SAMPLES: &str = "samples";
    const COUNT: &str = "count";
    const CPU: &str = "cpu";
    const NANOSECONDS: &str = "nanoseconds";
    const THREAD: &str = "thread";

    impl Report {
        /// Create a `Report` from a protobuf `Profile`. This can be useful
        /// for creating a flamegraph from a saved protobuf file.
        pub fn from_pprof(profile: &protos::Profile) -> crate::Result<Self> {
            let mut data = HashMap::new();

            let strings: Vec<&str> = profile.string_table.iter().map(|s| s.as_str()).collect();

            let mut functions = HashMap::new();
            for func in profile.function.iter() {
                functions.insert(func.id, func);
            }

            let mut locations = HashMap::new();
            for loc in profile.location.iter() {
                locations.insert(loc.id, loc);
            }

            for sample in profile.sample.iter() {
                let mut frames = Vec::new();

                for &loc_id in sample.location_id.iter() {
                    if let Some(location) = locations.get(&loc_id) {
                        let mut symbols = Vec::new();

                        for line in location.line.iter() {
                            if let Some(function) = functions.get(&line.function_id) {
                                let name =
                                    strings.get(function.name as usize).unwrap_or(&"Unknown");
                                let filename = strings
                                    .get(function.filename as usize)
                                    .unwrap_or(&"Unknown");

                                let symbol = crate::Symbol {
                                    name: Some(name.as_bytes().to_vec()),
                                    addr: None,
                                    lineno: if line.line > 0 {
                                        Some(line.line as u32)
                                    } else {
                                        None
                                    },
                                    filename: if *filename != "Unknown" {
                                        Some(filename.into())
                                    } else {
                                        None
                                    },
                                };
                                symbols.push(symbol);
                            }
                        }

                        if !symbols.is_empty() {
                            frames.push(symbols);
                        }
                    }
                }

                // Extract thread name from labels
                let mut thread_name = String::new();
                for label in sample.label.iter() {
                    let key_str = strings.get(label.key as usize).unwrap_or(&"");
                    if *key_str == THREAD {
                        thread_name = strings.get(label.str as usize).unwrap_or(&"").to_string();
                        break;
                    }
                }

                let frames_key = Frames {
                    frames,
                    thread_name,
                    thread_id: 0, // Not preserved in protobuf format
                    sample_timestamp: SystemTime::UNIX_EPOCH, // Not preserved
                };

                let count = sample.value.first().copied().unwrap_or(0) as isize;
                *data.entry(frames_key).or_insert(0) += count;
            }

            let frequency = if profile.period > 0 {
                (1_000_000_000 / profile.period) as i32
            } else {
                1
            };

            let start_time = if profile.time_nanos > 0 {
                SystemTime::UNIX_EPOCH + std::time::Duration::from_nanos(profile.time_nanos as u64)
            } else {
                SystemTime::UNIX_EPOCH
            };

            let duration = if profile.duration_nanos > 0 {
                std::time::Duration::from_nanos(profile.duration_nanos as u64)
            } else {
                std::time::Duration::default()
            };

            let timing = crate::timer::ReportTiming {
                frequency,
                start_time,
                duration,
            };

            Ok(Report { data, timing })
        }

        /// `pprof` will generate google's pprof format report.
        pub fn pprof(&self) -> crate::Result<protos::Profile> {
            let mut dedup_str = HashSet::new();
            for key in self.data.keys() {
                dedup_str.insert(key.thread_name_or_id());
                for frame in key.frames.iter() {
                    for symbol in frame {
                        dedup_str.insert(symbol.name());
                        dedup_str.insert(symbol.sys_name().into_owned());
                        dedup_str.insert(symbol.filename().into_owned());
                    }
                }
            }
            dedup_str.insert(SAMPLES.into());
            dedup_str.insert(COUNT.into());
            dedup_str.insert(CPU.into());
            dedup_str.insert(NANOSECONDS.into());
            dedup_str.insert(THREAD.into());
            // string table's first element must be an empty string
            let mut str_tbl = vec!["".to_owned()];
            str_tbl.extend(dedup_str.into_iter());

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
                    let location_id = loc_tbl.len() as u64 + 1;
                    let mut lines = vec![];

                    for symbol in frame {
                        let name = symbol.name();
                        let function_id = if let Some(&existing_id) = functions.get(&name) {
                            existing_id
                        } else {
                            let sys_name = symbol.sys_name();
                            let filename = symbol.filename();
                            let function_id = fn_tbl.len() as u64 + 1;
                            let function = protos::Function {
                                id: function_id,
                                name: *strings.get(name.as_str()).unwrap() as i64,
                                system_name: *strings.get(sys_name.as_ref()).unwrap() as i64,
                                filename: *strings.get(filename.as_ref()).unwrap() as i64,
                                ..protos::Function::default()
                            };
                            functions.insert(name, function_id);
                            fn_tbl.push(function);
                            function_id
                        };

                        let lineno = symbol.lineno();
                        let line = protos::Line {
                            function_id,
                            line: lineno as i64,
                            ..protos::Line::default()
                        };
                        lines.push(line);
                    }

                    let loc = protos::Location {
                        id: location_id,
                        line: lines.into(),
                        ..protos::Location::default()
                    };
                    loc_tbl.push(loc);
                    locs.push(location_id);
                }
                let thread_name = protos::Label {
                    key: *strings.get(THREAD).unwrap() as i64,
                    str: *strings.get(&key.thread_name_or_id().as_str()).unwrap() as i64,
                    ..protos::Label::default()
                };
                let sample = protos::Sample {
                    location_id: locs,
                    value: vec![
                        *count as i64,
                        *count as i64 * 1_000_000_000 / self.timing.frequency as i64,
                    ],
                    label: vec![thread_name].into(),
                    ..Default::default()
                };
                samples.push(sample);
            }
            let samples_value = protos::ValueType {
                ty: *strings.get(SAMPLES).unwrap() as i64,
                unit: *strings.get(COUNT).unwrap() as i64,
                ..Default::default()
            };
            let time_value = protos::ValueType {
                ty: *strings.get(CPU).unwrap() as i64,
                unit: *strings.get(NANOSECONDS).unwrap() as i64,
                ..Default::default()
            };
            let profile = protos::Profile {
                sample_type: vec![samples_value, time_value.clone()].into(),
                sample: samples.into(),
                string_table: str_tbl.into(),
                function: fn_tbl.into(),
                location: loc_tbl.into(),
                time_nanos: self
                    .timing
                    .start_time
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as i64,
                duration_nanos: self.timing.duration.as_nanos() as i64,
                period_type: Some(time_value).into(),
                period: 1_000_000_000 / self.timing.frequency as i64,
                ..protos::Profile::default()
            };
            Ok(profile)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::collections::HashSet;

        #[test]
        fn test_roundtrip_conversion() {
            let guard = crate::ProfilerGuard::new(100).unwrap();

            // Generate profiling data with different call patterns
            for i in 0..100000 {
                if i % 3 == 0 {
                    expensive_function_a(i);
                } else if i % 3 == 1 {
                    expensive_function_b(i);
                } else {
                    expensive_function_c(i);
                }
            }

            let report = guard.report().build().unwrap();
            assert!(
                !report.data.is_empty(),
                "Should have captured some profiling data"
            );

            let profile = report.pprof().unwrap();
            let restored_report = Report::from_pprof(&profile).unwrap();

            let original_symbols: HashSet<String> = report
                .data
                .keys()
                .flat_map(|frames| frames.frames.iter())
                .flat_map(|frame| frame.iter())
                .map(|symbol| symbol.name())
                .collect();

            let restored_symbols: HashSet<String> = restored_report
                .data
                .keys()
                .flat_map(|frames| frames.frames.iter())
                .flat_map(|frame| frame.iter())
                .map(|symbol| symbol.name())
                .collect();

            assert_eq!(original_symbols.len(), restored_symbols.len());
            for symbol in &original_symbols {
                assert!(restored_symbols.contains(symbol));
            }

            let original_total: isize = report.data.values().sum();
            let restored_total: isize = restored_report.data.values().sum();
            assert_eq!(original_total, restored_total);

            assert_eq!(report.timing.frequency, restored_report.timing.frequency);
        }

        #[inline(never)]
        fn expensive_function_a(n: usize) -> usize {
            (0..n % 100).map(|i| i * i).sum()
        }

        #[inline(never)]
        fn expensive_function_b(n: usize) -> usize {
            (0..n % 50).fold(1, |acc, x| acc.wrapping_mul(x + 1))
        }

        #[inline(never)]
        fn expensive_function_c(n: usize) -> usize {
            let mut result = n;
            for i in 0..n % 30 {
                result = result.wrapping_add(i * 3);
            }
            result
        }
    }
}
