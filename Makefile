export RUSTFLAGS=-Dwarnings

.PHONY: default check unit-test integration-tests test doc docker-pd docker-kv docker all

PD_ADDRS ?= "127.0.0.1:2379"
MULTI_REGION ?= 1

ALL_FEATURES := integration-tests

INTEGRATION_TEST_ARGS := --features "integration-tests"

default: check

check:
	cargo check --all --all-targets --features "${ALL_FEATURES}"
	cargo fmt -- --check
	cargo clippy --all-targets --features "${ALL_FEATURES}" -- -D clippy::all

unit-test:
	cargo test --all --no-default-features

integration-test:
	cargo test txn_ --all ${INTEGRATION_TEST_ARGS} -- --nocapture
	cargo test raw_ --all ${INTEGRATION_TEST_ARGS} -- --nocapture
	cargo test misc_ --all ${INTEGRATION_TEST_ARGS} -- --nocapture

test: unit-test integration-test

doc: 
	cargo doc --workspace --exclude tikv-client-proto --document-private-items --no-deps

# Deprecated
# docker-pd:
# 	docker run -d -v $(shell pwd)/config:/config --net=host --name pd --rm pingcap/pd:latest --name "pd" --data-dir "pd" --client-urls "http://127.0.0.1:2379" --advertise-client-urls "http://127.0.0.1:2379" --config /config/pd.toml

# docker-kv:
# 	docker run -d -v $(shell pwd)/config:/config --net=host --name kv --rm --ulimit nofile=90000:90000 pingcap/tikv:latest --pd-endpoints "127.0.0.1:2379" --addr "127.0.0.1:2378" --data-dir "kv" --config /config/tikv.toml

# docker: docker-pd docker-kv

tiup:
	tiup playground nightly --mode tikv-slim --kv 3 --without-monitor --kv.config $(shell pwd)/config/tikv.toml --pd.config $(shell pwd)/config/pd.toml &

all: check doc test

clean:
	cargo clean
	rm -rf target
