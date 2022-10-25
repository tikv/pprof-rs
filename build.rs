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

#[cfg(feature = "prost-codec")]
fn generate_prost() {
    use sha2::{Digest, Sha256};
    use std::{
        fmt::Write,
        fs::{self, File},
        io::{self, BufRead, BufReader},
    };

    const PRE_GENERATED_PATH: &str = "proto/perftools.profiles.rs";

    // Calculate the SHA256 of the proto file
    let mut hasher = Sha256::new();
    let mut proto_file = BufReader::new(File::open("proto/profile.proto").unwrap());
    io::copy(&mut proto_file, &mut hasher).unwrap();
    let mut hex = String::new();
    for b in hasher.finalize() {
        write!(&mut hex, "{:x}", b).unwrap();
    }
    let hash_comment = format!("// {}  proto/profile.proto", hex);

    let first_line = File::open(PRE_GENERATED_PATH)
        .and_then(|f| {
            let mut reader = BufReader::new(f);
            let mut first_line = String::new();
            reader.read_line(&mut first_line)?;
            Ok(first_line)
        })
        .unwrap_or_default();
    // If the hash of the proto file changes, regenerate the prost file.
    if first_line.trim() != hash_comment {
        prost_build::Config::new()
            .out_dir("proto/")
            .compile_protos(&["proto/profile.proto"], &["proto/"])
            .unwrap();
        // Prepend the hash comment to the generated file.
        let generated = fs::read_to_string(PRE_GENERATED_PATH).unwrap();
        let with_hex = format!("{}\n\n{}", hash_comment, generated);
        fs::write(PRE_GENERATED_PATH, with_hex).unwrap();
    }
}

fn main() {
    #[cfg(feature = "prost-codec")]
    generate_prost();
    #[cfg(feature = "protobuf-codec")]
    generate_protobuf();
}
