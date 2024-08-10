export RUSTFLAGS=-Dwarnings

.PHONY: default check unit-test integration-tests test doc docker-pd docker-kv docker all

export PD_ADDRS     ?= 127.0.0.1:2379
export MULTI_REGION ?= 1

ALL_FEATURES := integration-tests

INTEGRATION_TEST_ARGS := --features "integration-tests"

default: check

generate:
	cargo run -p tikv-client-proto-build

check: generate
	cargo check --all --all-targets --features "${ALL_FEATURES}"
	cargo fmt -- --check
	cargo clippy --all-targets --features "${ALL_FEATURES}" -- -D clippy::all

unit-test: generate
	cargo nextest run --all --no-default-features

integration-test: generate
	cargo test txn_ --all ${INTEGRATION_TEST_ARGS} -- --nocapture
	cargo test raw_ --all ${INTEGRATION_TEST_ARGS} -- --nocapture
	cargo test misc_ --all ${INTEGRATION_TEST_ARGS} -- --nocapture

test: unit-test integration-test

doc:
	cargo doc --workspace --exclude tikv-client-proto --document-private-items --no-deps

tiup:
	tiup playground nightly --mode tikv-slim --kv 3 --without-monitor --kv.config $(shell pwd)/config/tikv.toml --pd.config $(shell pwd)/config/pd.toml &

all: generate check doc test

clean:
	cargo clean
	rm -rf target
