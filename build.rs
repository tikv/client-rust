// Copyright 2020 TiKV Project Authors. Licensed under Apache-2.0.

fn main() {
    #[cfg(feature = "vendored")]
    {
        use tonic_disable_doctest::BuilderEx;

        tonic_build::configure()
            .disable_doctests_for_types([".google.api.HttpRule"])
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
}
