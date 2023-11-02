// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

fn main() {
    tonic_build::configure()
        .emit_rerun_if_changed(false)
        .build_server(false)
        .include_file("mod.rs")
        .out_dir("src/generated")
        .compile(
            &glob::glob("proto/*.proto")
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            &["proto/include", "proto"],
        )
        .unwrap();
}
