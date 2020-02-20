// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

fn main() {
    #[cfg(feature = "prost-protobuf")]
    prost_build::compile_protos(&["proto/profile.proto"], &["proto/"]).unwrap();

    #[cfg(feature = "rust-protobuf")]
    protoc_rust::run(protoc_rust::Args {
        out_dir: "src/",
        input: &["proto/profile.proto"],
        includes: &["proto/"],
        customize: protoc_rust::Customize {
            ..Default::default()
        },
    })
    .expect("protoc");
}
