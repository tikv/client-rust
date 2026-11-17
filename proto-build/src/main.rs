// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

use std::path::PathBuf;

fn main() {
    let protos = glob::glob("proto/*.proto")
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let includes = [PathBuf::from("proto/include"), PathBuf::from("proto")];

    tonic_prost_build::configure()
        .emit_rerun_if_changed(false)
        .build_server(false)
        .include_file("mod.rs")
        .out_dir("src/generated")
        .compile_protos(&protos, &includes)
        .unwrap();
}
