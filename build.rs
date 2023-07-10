// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

use tonic_disable_doctest::BuilderEx;

fn main() {
    tonic_build::configure()
        .disable_doctests_for_types([".google.api.HttpRule"])
        .build_server(false)
        .include_file("mod.rs")
        .compile(
            &glob::glob("proto/*.proto")
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap(),
            &["proto/include", "proto"],
        )
        .unwrap();
}
