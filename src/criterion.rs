// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#[cfg(feature = "flamegraph")]
use crate::flamegraph::Options as FlamegraphOptions;
#[cfg(feature = "protobuf")]
use crate::protos::Message;

use crate::ProfilerGuard;
use criterion::profiler::Profiler;

use std::fs::File;
#[cfg(feature = "protobuf")]
use std::io::Write;
use std::os::raw::c_int;
use std::path::Path;

pub enum Output<'a> {
    #[cfg(feature = "flamegraph")]
    Flamegraph(Option<FlamegraphOptions<'a>>),

    #[cfg(feature = "protobuf")]
    Protobuf,
}

pub struct PProfProfiler<'a, 'b> {
    frequency: c_int,
    output: Output<'b>,
    active_profiler: Option<ProfilerGuard<'a>>,
}

impl<'a, 'b> PProfProfiler<'a, 'b> {
    pub fn new(frequency: c_int, output: Output<'b>) -> Self {
        Self {
            frequency,
            output,
            active_profiler: None,
        }
    }
}

impl<'a, 'b> Profiler for PProfProfiler<'a, 'b> {
    fn start_profiling(&mut self, _benchmark_id: &str, _benchmark_dir: &Path) {
        self.active_profiler = Some(ProfilerGuard::new(self.frequency).unwrap());
    }

    fn stop_profiling(&mut self, benchmark_id: &str, benchmark_dir: &Path) {
        std::fs::create_dir_all(benchmark_dir).unwrap();

        let ext = match self.output {
            #[cfg(feature = "flamegraph")]
            Output::Flamegraph(_) => ".svg",
            #[cfg(feature = "protobuf")]
            Output::Protobuf => ".pb",
        };
        let output_path = benchmark_dir.join(format!("{}{}", benchmark_id, ext));
        let output_file = File::create(&output_path).unwrap_or_else(|_| {
            panic!("File system error while creating {}", output_path.display())
        });

        if let Some(profiler) = self.active_profiler.take() {
            match &mut self.output {
                #[cfg(feature = "flamegraph")]
                Output::Flamegraph(options) => {
                    let default_options = &mut FlamegraphOptions::default();
                    let options = options.as_mut().unwrap_or(default_options);

                    profiler
                        .report()
                        .build()
                        .unwrap()
                        .flamegraph_with_options(output_file, options)
                        .expect("Error while writing flamegraph");
                }

                #[cfg(feature = "protobuf")]
                Output::Protobuf => {
                    let mut output_file = output_file;

                    let profile = profiler.report().build().unwrap().pprof().unwrap();

                    let mut content = Vec::new();
                    profile
                        .encode(&mut content)
                        .expect("Error while encoding protobuf");

                    output_file
                        .write_all(&content)
                        .expect("Error while writing protobuf");
                }
            }
        }
    }
}
