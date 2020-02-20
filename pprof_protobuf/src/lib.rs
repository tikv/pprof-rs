// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![allow(dead_code)]

#[cfg(feature = "prost-protobuf")]
mod inner {
    pub use prost::Message;

    include!(concat!(env!("OUT_DIR"), "/perftools.profiles.rs"));
}

#[cfg(feature = "rust-protobuf")]
mod profile;
#[cfg(feature = "rust-protobuf")]
mod inner {
    pub use protobuf::Message;

    pub use super::profile::*;
}

mod protobuf_codec;

pub use inner::Message;
pub use inner::Profile as ProfileProtobuf;
pub use protobuf_codec::*;
