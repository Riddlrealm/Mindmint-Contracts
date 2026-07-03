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

# Deploy a single contract by package name. Usage: make deploy-testnet CONTRACT=puzzle_verification
deploy-testnet:
	@if [ -z "$(CONTRACT)" ]; then \
		echo "Usage: make deploy-testnet CONTRACT=<package-name>"; \
		echo "Available packages:"; \
		cargo metadata --format-version=1 --no-deps 2>/dev/null | grep -oE '"name":"[^"]+"' | sed 's/"name":"//;s/"//' | sort; \
		exit 1; \
	fi
	@echo "Deploying $(CONTRACT) to testnet..."
	soroban contract build --package $(CONTRACT)
	soroban contract optimize --wasm target/wasm32-unknown-unknown/release/$(CONTRACT).wasm
	soroban contract deploy \
		--wasm target/wasm32-unknown-unknown/release/$(CONTRACT).optimized.wasm \
		--source deployer \
		--network testnet
