.PHONY: default check unit-test integration-tests test doc all

default: check

check:
	cargo check --all
	cargo fmt -- --check
	cargo clippy -- -D clippy::all

unit-test:
	cargo test --all

integration-test:
# MULTI_REGION shall be set manually if needed
	PD_ADDRS="127.0.0.1:2379" cargo test txn_ --all --features integration-tests -- --nocapture
	PD_ADDRS="127.0.0.1:2379" cargo test raw_ --all --features integration-tests -- --nocapture
	PD_ADDRS="127.0.0.1:2379" cargo test misc_ --all --features integration-tests -- --nocapture

test: unit-test integration-test

doc: 
	cargo doc --workspace --exclude tikv-client-proto --document-private-items --no-deps

all: check doc test
