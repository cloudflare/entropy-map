.PHONY: clean build test check-fmt fmt lint

clean:
	rm -rf ./target/

build:
	cargo build --release

test:
	cargo test

check-fmt:
	cargo fmt -- --check

fmt:
	cargo fmt

lint:
	cargo clippy
	cargo check --workspace --benches
