// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.
#[cfg(feature = "protobuf-codec")]
// Allow deprecated as TiKV pin versions to a outdated one.
fn generate_protobuf() {
    use std::io::Write;
    let customize = <protobuf_codegen::Customize as std::default::Default>::default();
    // Set the output directory for generated files
    let out_dir = std::env::var("OUT_DIR").unwrap();

    let mut cg = protobuf_codegen::Codegen::new();
    cg.pure();

    cg.inputs(["proto/profile.proto"]).includes(["proto"]);

    cg.customize(customize);
    cg.out_dir(&out_dir).run().unwrap();

    // Optionally, write a mod.rs file for module inclusion
    let mut f = std::fs::File::create(format!("{}/mod.rs", out_dir)).unwrap();
    write!(f, "pub mod profile;").unwrap();
}

// Runs `generate` if the file at `pre_generated_path` is missing, or if the hash comment on its
// first line does not match the proto file. `generate` must write that file. To regenerate after
// upgrading the code generator, delete the file.
#[cfg(any(feature = "prost-codec", feature = "buffa-codec"))]
fn regenerate_if_stale(pre_generated_path: &str, generate: impl FnOnce()) {
    use sha2::{Digest, Sha256};
    use std::{
        fmt::Write,
        fs::{self, File},
        io::{self, BufRead, BufReader},
    };

    // Calculate the SHA256 of the proto file
    let mut hasher = Sha256::new();
    let mut proto_file = BufReader::new(File::open("proto/profile.proto").unwrap());
    io::copy(&mut proto_file, &mut hasher).unwrap();
    let mut hex = String::new();
    for b in hasher.finalize() {
        write!(&mut hex, "{:x}", b).unwrap();
    }
    let hash_comment = format!("// {}  proto/profile.proto", hex);

    let first_line = File::open(pre_generated_path)
        .and_then(|f| {
            let mut reader = BufReader::new(f);
            let mut first_line = String::new();
            reader.read_line(&mut first_line)?;
            Ok(first_line)
        })
        .unwrap_or_default();
    // If the hash of the proto file changes, regenerate the file.
    if first_line.trim() != hash_comment {
        // buffa-build prints rerun-if-changed for the proto file, so cargo stops watching the
        // rest of the package. Watch the generated file too.
        println!("cargo:rerun-if-changed=proto/profile.proto");
        println!("cargo:rerun-if-changed={}", pre_generated_path);
        generate();
        // Prepend the hash comment to the generated file.
        let generated = fs::read_to_string(pre_generated_path).unwrap();
        let with_hex = format!("{}\n\n{}", hash_comment, generated);
        fs::write(pre_generated_path, with_hex).unwrap();
    }
}

#[cfg(feature = "prost-codec")]
fn generate_prost() {
    regenerate_if_stale("proto/perftools.profiles.rs", || {
        prost_build::Config::new()
            .out_dir("proto/")
            .compile_protos(&["proto/profile.proto"], &["proto/"])
            .unwrap();
    });
}

#[cfg(feature = "buffa-codec")]
fn generate_buffa() {
    // buffa-build and prost-build both name the file after the proto package,
    // so the buffa file has its own directory.
    regenerate_if_stale("proto/buffa/perftools.profiles.rs", || {
        buffa_build::Config::new()
            .files(&["proto/profile.proto"])
            .includes(&["proto/"])
            .out_dir("proto/buffa/")
            .generate_views(false)
            .file_per_package(true)
            .compile()
            .unwrap();
    });
}

fn main() {
    #[cfg(feature = "prost-codec")]
    generate_prost();
    #[cfg(feature = "protobuf-codec")]
    generate_protobuf();
    #[cfg(feature = "buffa-codec")]
    generate_buffa();
}
