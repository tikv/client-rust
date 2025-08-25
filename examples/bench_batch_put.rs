// Copyright 2018 TiKV Project Authors. Licensed under Apache-2.0.

#![type_length_limit = "8165158"]

mod common;

use std::time::Instant;
use tikv_client::Config;
use tikv_client::KvPair;
use tikv_client::RawClient as Client;
use tikv_client::Result;

use crate::common::parse_args;

const TARGET_SIZE_MB: usize = 40;
const KEY_SIZE: usize = 32;
const VALUE_SIZE: usize = 1024; // 1KB per value

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    // Parse command line arguments
    let args = parse_args("raw");

    // Create a configuration to use for the example
    let config = if let (Some(ca), Some(cert), Some(key)) = (args.ca, args.cert, args.key) {
        Config::default().with_security(ca, cert, key)
    } else {
        Config::default()
    };

    // Create the client
    let client = Client::new_with_config(args.pd, config).await?;

    // Calculate how many key-value pairs we need to reach 100MB
    let pair_size = KEY_SIZE + VALUE_SIZE;
    let target_size_bytes = TARGET_SIZE_MB * 1024 * 1024;
    let num_pairs = target_size_bytes / pair_size;

    println!("Preparing to create {} key-value pairs", num_pairs);
    println!("Key size: {} bytes, Value size: {} bytes", KEY_SIZE, VALUE_SIZE);
    println!("Total data size: ~{} MB", (num_pairs * pair_size) / (1024 * 1024));

    // Generate key-value pairs
    println!("Generating key-value pairs...");
    let generation_start = Instant::now();

    let mut pairs = Vec::with_capacity(num_pairs);
    for i in 0..num_pairs {
        // Generate key: "bench_key_" + zero-padded number
        let key = format!("bench_key_{:010}", i);

        // Generate value: repeat pattern to reach VALUE_SIZE
        let pattern = format!("value_{}", i % 1000);
        let mut value = String::new();
        while value.len() < VALUE_SIZE {
            value.push_str(&pattern);
        }
        value.truncate(VALUE_SIZE);

        pairs.push(KvPair::from((key, value)));
    }

    let generation_duration = generation_start.elapsed();
    println!("Generated {} pairs in {:?}", pairs.len(), generation_duration);

    // Perform batch_put and measure timing
    println!("Starting batch_put operation...");
    let batch_put_start = Instant::now();

    client.batch_put(pairs).await.expect("Failed to perform batch_put");

    let batch_put_duration = batch_put_start.elapsed();

    // Calculate statistics
    let total_bytes = num_pairs * pair_size;
    let throughput_mb_per_sec = (total_bytes as f64 / (1024.0 * 1024.0)) / batch_put_duration.as_secs_f64();
    let ops_per_sec = num_pairs as f64 / batch_put_duration.as_secs_f64();

    // Print results
    println!("\n=== Batch Put Benchmark Results ===");
    println!("Total key-value pairs: {}", num_pairs);
    println!("Total data size: {:.2} MB", total_bytes as f64 / (1024.0 * 1024.0));
    println!("Batch put duration: {:?}", batch_put_duration);
    println!("Throughput: {:.2} MB/s", throughput_mb_per_sec);
    println!("Operations per second: {:.2} ops/s", ops_per_sec);
    println!("Average latency per operation: {:.2} μs", batch_put_duration.as_micros() as f64 / num_pairs as f64);

    Ok(())
}
