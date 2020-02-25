// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(feature = "prost-codec")]
fn build(dest_path: PathBuf) {
    protobuf_build::Builder::new()
        .append_include("proto/".to_owned())
        .files(&["proto/profile.proto"])
        .out_dir(dest_path.to_str().unwrap())
        .generate_files();
}

#[cfg(feature = "protobuf-codec")]
fn build(dest_path: PathBuf) {
    use std::fs::File;
    use std::io::Write;

    protobuf_build::Builder::new()
        .append_include("proto/".to_owned())
        .files(&["proto/profile.proto"])
        .out_dir(dest_path.to_str().unwrap())
        .generate_files();

    let mut mod_file = File::create(dest_path.join("mod.rs")).unwrap();
    mod_file
        .write_all(b"mod profile;pub use profile::*;")
        .expect("Unable to write mod file");
}

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("protos");
    fs::create_dir_all(&dest_path).unwrap();

    build(dest_path);
}
