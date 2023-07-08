// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

fn main() {
    tonic_build::configure()
        .build_server(false)
        .include_file("mod.rs")
        .compile(
            &glob::glob("proto/*.proto")
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            &["include", "proto"],
        )
        .unwrap();
}
