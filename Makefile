.PHONY: build test clean optimize check fmt

build:
	soroban contract build

test:
	cargo test

clean:
	cargo clean
	rm -rf target/

optimize:
	soroban contract optimize --wasm target/wasm32-unknown-unknown/release/*.wasm

check:
	cargo check --workspace --all-targets
	cargo clippy --workspace --all-targets -- -D warnings

fmt:
	cargo fmt --all
	cargo fmt --all -- --check

# See docs/DEPLOYMENT.md for full deployment instructions.
