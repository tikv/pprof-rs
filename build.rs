fn main() {
    prost_build::compile_protos(&["proto/profile.proto"],
                                &["proto/"]).unwrap();
}