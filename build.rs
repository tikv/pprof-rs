fn main() {
    #[cfg(feature = "protobuf")]
    prost_build::compile_protos(&["proto/profile.proto"], &["proto/"]).unwrap();
}
