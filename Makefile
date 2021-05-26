.PHONY: default check unit-test integration-tests test doc docker-pd docker-kv docker all

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

docker-pd:
	docker run -d -v $(shell pwd)/config:/config --net=host --name pd --rm pingcap/pd:latest --name "pd" --data-dir "pd" --client-urls "http://127.0.0.1:2379" --advertise-client-urls "http://127.0.0.1:2379" --config /config/pd.toml

docker-kv:
	docker run -d -v $(shell pwd)/config:/config --net=host --name kv --rm --ulimit nofile=90000:90000 pingcap/tikv:latest --pd-endpoints "127.0.0.1:2379" --addr "127.0.0.1:2378" --data-dir "kv" --config /config/tikv.toml

docker: docker-pd docker-kv

tiup:
	tiup playground nightly --db 0 --tiflash 0 --monitor false --kv.config $(shell pwd)/config/tikv.toml --pd.config $(shell pwd)/config/pd.toml &

all: check doc test
