// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

#![allow(dead_code)]

#[cfg(feature = "prost-codec")]
mod inner {
    pub use prost::Message;

    include!(concat!(env!("OUT_DIR"), "/protos/perftools.profiles.rs"));
}

#[cfg(feature = "protobuf-codec")]
mod profile;
#[cfg(feature = "protobuf-codec")]
mod inner {
    pub use protobuf::Message;

    pub use super::profile::*;
}

mod protobuf_codec;

pub use inner::Message;
pub use inner::Profile as ProfileProtobuf;
pub use protobuf_codec::*;
