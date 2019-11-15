// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

fn main() {
    #[cfg(feature = "protobuf")]
    prost_build::compile_protos(&["proto/profile.proto"], &["proto/"]).unwrap();
}
