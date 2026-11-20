.DEFAULT_GOAL := help
FORMAT_RUST_ARGS := --extra-workspace qualification --rustfmt rustfmt
CARGO_PUBLISH_PACKAGES := ic-storage
CARGO_PUBLISH_REGISTRY := crates-io
CARGO_PUBLISH_ALL_FEATURES := 1
override CARGO_PUBLISH_VERIFY_TARGETS := lint deps-check fmt-check test doc-check qualification-lint storage-installed-test
publish-check: override CARGO_PUBLISH_VERIFY_TARGETS := lint
include vendor/toko-tooling/make/consumer.mk
include vendor/toko-tooling/make/cargo-dependencies.mk

export RUSTUP_AUTO_INSTALL := 0
PRODUCT := -p ic-storage --locked
QUAL := --manifest-path qualification/Cargo.toml --target-dir target/qualification --locked
SERVER_TOOL := bash vendor/toko-tooling/scripts/dev/install-rust-tools.sh --consumer "$(CURDIR)" --package ic-testkit --version 0.110.0 --bin ic-testkit-server --profile release --features cli
SERVER_DIRECTORY := $(CURDIR)/.tools/testkit-server

.PHONY: help check lint test doc-check ci deps-check deps-pins deps-boundaries storage-build storage-setup storage-installed-test qualification-lint
help:
	@printf '%s\n' 'make test                    Host allocation/collection and ownership tests' 'make lint                    Product host/Wasm Clippy' 'make storage-setup           Explicit Testkit server setup' 'make storage-installed-test  Installed upgrade and rollback tests' 'make ci                      Aggregate local CI checks'

check lint test doc-check deps-check deps-pins deps-boundaries storage-build storage-installed-test qualification-lint: | toko-execution-check
check:
	+cargo check $(PRODUCT)
lint:
	+cargo clippy $(PRODUCT) --all-targets --all-features -- -D warnings
	+cargo clippy $(PRODUCT) --lib --target wasm32-unknown-unknown -- -D warnings
test:
	+cargo test $(PRODUCT) --lib --tests
	+cargo test $(PRODUCT) --doc
doc-check:
	+RUSTDOCFLAGS='-D warnings' cargo doc $(PRODUCT) --all-features --no-deps
qualification-lint:
	+cargo clippy $(QUAL) -p storage-tests --all-targets -- -D warnings
	+cargo clippy $(QUAL) -p storage-canister --target wasm32-unknown-unknown -- -D warnings
ci: fmt-check deps-check lint test doc-check qualification-lint

deps-check: toko-tooling-check deps-pins deps-boundaries
deps-pins:
	+bash "$(TOKO_TOOLING_ROOT)/scripts/ci/check-dependency-pins.sh" --consumer "$(CURDIR)" --cargo-inheritance
deps-boundaries:
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)" --package ic-storage --target all --depth 1 --require ic-platform=1 --forbid ic-stable-structures
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)" --package ic-storage --target wasm32-unknown-unknown --require ic-stable-structures=1 --require ic-platform-runtime=1 --forbid ic-wire --forbid ic-memory --forbid ic-storage-derive --forbid ic-storage-m0 --forbid toko-tooling

storage-build:
	+RUSTFLAGS='-C link-arg=-zstack-size=1048576' cargo build $(QUAL) -p storage-canister --target wasm32-unknown-unknown --release
storage-setup:
	+@umask 077; server="$$($(SERVER_TOOL))" && mkdir -p target/evidence/testkit && attempt="$$(mktemp -d "$(CURDIR)/target/evidence/testkit/setup.XXXXXX")" && "$$server" setup "$(SERVER_DIRECTORY)" "$$attempt/evidence" /usr/bin/curl /usr/bin/gzip
storage-installed-test: storage-build
	+@umask 077; server="$$($(SERVER_TOOL) --check)" && mkdir -p target/evidence/testkit && attempt="$$(mktemp -d "$(CURDIR)/target/evidence/testkit/run.XXXXXX")" && STORAGE_CANISTER_WASM="$(CURDIR)/target/qualification/wasm32-unknown-unknown/release/storage_canister.wasm" "$$server" run "$(SERVER_DIRECTORY)" "$$attempt/evidence" --ttl 300 -- cargo test $(QUAL) -p storage-tests --test lifecycle -- --ignored --nocapture
