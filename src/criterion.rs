// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#[cfg(feature = "flamegraph")]
use crate::flamegraph::Options as FlamegraphOptions;
#[cfg(feature = "_protobuf")]
use crate::protos::Message;

use crate::ProfilerGuard;
use criterion::profiler::Profiler;

use std::fs::File;
#[cfg(feature = "_protobuf")]
use std::io::Write;
use std::marker::PhantomData;
use std::os::raw::c_int;
use std::path::Path;

#[allow(clippy::large_enum_variant)]
pub enum Output<'a> {
    #[cfg(feature = "flamegraph")]
    Flamegraph(Option<FlamegraphOptions<'a>>),

    #[cfg(feature = "_protobuf")]
    Protobuf,

    #[deprecated(
        note = "This branch is used to include lifetime parameter. Don't use it directly."
    )]
    _Phantom(PhantomData<&'a ()>),
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

#[cfg(not(any(feature = "_protobuf", feature = "flamegraph")))]
compile_error!("Either feature \"protobuf\" or \"flamegraph\" must be enabled when \"criterion\" feature is enabled.");

impl<'a, 'b> Profiler for PProfProfiler<'a, 'b> {
    fn start_profiling(&mut self, _benchmark_id: &str, _benchmark_dir: &Path) {
        self.active_profiler = Some(ProfilerGuard::new(self.frequency).unwrap());
    }

    fn stop_profiling(&mut self, _benchmark_id: &str, benchmark_dir: &Path) {
        std::fs::create_dir_all(benchmark_dir).unwrap();

        let filename = match self.output {
            #[cfg(feature = "flamegraph")]
            Output::Flamegraph(_) => "flamegraph.svg",
            #[cfg(feature = "_protobuf")]
            Output::Protobuf => "profile.pb",
            // This is `""` but not `unreachable!()`, because `unreachable!()`
            // will result in another compile error, so that the user may not
            // realize the error thrown by `compile_error!()` at the first time.
            _ => "",
        };
        let output_path = benchmark_dir.join(filename);
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

                #[cfg(feature = "_protobuf")]
                Output::Protobuf => {
                    let mut output_file = output_file;

                    let profile = profiler.report().build().unwrap().pprof().unwrap();

                    let mut content = Vec::new();
                    #[cfg(not(feature = "protobuf-codec"))]
                    profile
                        .encode(&mut content)
                        .expect("Error while encoding protobuf");
                    #[cfg(feature = "protobuf-codec")]
                    profile
                        .write_to_vec(&mut content)
                        .expect("Error while encoding protobuf");

                    output_file
                        .write_all(&content)
                        .expect("Error while writing protobuf");
                }

                _ => unreachable!(),
            }
        }
    }
}
