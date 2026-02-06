// Copyright 2023 TiKV Project Authors. Licensed under Apache-2.0.

fn main() {
    let generated_mod = "src/generated/mod.rs";
    let dead_code_context = "// Rust 1.93's clippy::all now flags unused generated wire types as dead code; prior toolchain did not fail on these.";

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

    // Keep dead_code allowance scoped to generated modules only.
    let mod_rs = std::fs::read_to_string(generated_mod).unwrap();
    let mut mod_rs = mod_rs
        .replace(
            &format!("\n{dead_code_context}\n#[allow(dead_code)]\n"),
            "\n",
        )
        .replace("\n#[allow(dead_code)]\n", "\n");
    let dead_code_prefix = format!("{dead_code_context}\n#[allow(dead_code)]\n");
    mod_rs = mod_rs.replace("\npub mod ", &format!("\n{dead_code_prefix}pub mod "));
    if mod_rs.starts_with("pub mod ") {
        mod_rs = format!("{dead_code_prefix}{mod_rs}");
    }
    std::fs::write(generated_mod, mod_rs).unwrap();
}
