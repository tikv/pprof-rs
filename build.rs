// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

#[cfg(feature = "protobuf-codec")]
// Allow deprecated as TiKV pin versions to a outdated one.
#[allow(deprecated)]
fn generate_protobuf() {
    use std::io::Write;

    let out_dir = std::env::var("OUT_DIR").unwrap();
    protobuf_codegen_pure::run(protobuf_codegen_pure::Args {
        out_dir: &out_dir,
        includes: &["proto"],
        input: &["proto/profile.proto"],
        customize: protobuf_codegen_pure::Customize {
            generate_accessors: Some(false),
            lite_runtime: Some(true),
            ..Default::default()
        },
    })
    .unwrap();
    let mut f = std::fs::File::create(format!("{}/mod.rs", out_dir)).unwrap();
    write!(f, "pub mod profile;").unwrap();
}

fn main() {
    #[cfg(feature = "prost-codec")]
    prost_build::compile_protos(&["proto/profile.proto"], &["proto/"]).unwrap();
    #[cfg(feature = "protobuf-codec")]
    generate_protobuf();
}
