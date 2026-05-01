.PHONY: help build test run-server build-wasm dev-web build-web format lint check clean

help:
	@echo "Available targets:"
	@echo "  build       - cargo build"
	@echo "  test        - cargo test"
	@echo "  run-server  - run the axum HTTP server"
	@echo "  build-wasm  - build WASM and copy to web/public/pkg/"
	@echo "  dev-web     - run Astro dev server"
	@echo "  build-web   - build WASM + Astro for production"
	@echo "  format      - cargo fmt --all"
	@echo "  lint        - cargo clippy --all-targets --all-features -- -D warnings"
	@echo "  check       - format + lint + test (full local check)"
	@echo "  clean       - remove build artifacts"

build:
	cargo build

test:
	cargo test

run-server:
	cargo run --bin zkp

build-wasm:
	wasm-pack build --target web --no-default-features --features wasm
	rm -rf web/public/pkg
	mkdir -p web/public
	cp -r pkg web/public/pkg

dev-web: build-wasm
	cd web && npm run dev

build-web: build-wasm
	cd web && npm run build

format:
	cargo fmt --all

lint:
	cargo clippy --all-targets --all-features -- -D warnings

check: format lint test

clean:
	cargo clean
	rm -rf pkg web/public/pkg web/dist web/.astro
