// Copyright 2019 TiKV Project Authors. Licensed under Apache-2.0.

#[cfg(feature = "protobuf")]
fn generate_protobuf_binding_file() {
    prost_build::compile_protos(&["proto/proto_model.proto"], &["proto"]).unwrap();
}

#[cfg(not(feature = "protobuf"))]
fn generate_protobuf_binding_file() {}

fn main() {
    generate_protobuf_binding_file()
}
