// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

fn main() {
    use tonic_disable_doctest::BuilderEx;

    tonic_build::configure()
        .disable_doctests_for_types([".google.api.HttpRule"])
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
