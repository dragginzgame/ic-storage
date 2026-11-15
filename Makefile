.DEFAULT_GOAL := help
FORMAT_RUST_ARGS := --extra-workspace qualification

RETENTION_CARGO := --manifest-path qualification/Cargo.toml -p ic-storage-retention-probe --target-dir target/qualification --locked --offline
BENCH_CARGO := --manifest-path qualification/Cargo.toml -p ic-storage-simple-benchmarks --locked --offline
NATIVE_FEASIBILITY_CARGO := --manifest-path qualification/Cargo.toml -p ic-storage-native-feasibility --target-dir target/qualification --locked --offline
CHECKPOINT_CARGO := --manifest-path qualification/Cargo.toml -p ic-storage-checkpoint-provider-probe --target wasm32-unknown-unknown --locked --offline --release
CHECKPOINT_LINK := -C link-arg=--stack-first -C link-arg=-zstack-size=1048576 -C link-arg=--initial-memory=2097152 -C link-arg=--max-memory=16777216
RETENTION_TOOL := bash vendor/toko-tooling/scripts/dev/install-rust-tools.sh --consumer "$(CURDIR)" --package ic-testkit --lockfile qualification/Cargo.lock --bin ic-testkit-server --profile release --features cli
RUST_STARTUP_CARGO := --manifest-path qualification/Cargo.toml -p ic-storage-rust-startup-probe --target wasm32-unknown-unknown --locked --offline --release
RUST_STARTUP_LINK := -C link-arg=--stack-first -C link-arg=-zstack-size=1048576 -C link-arg=--initial-memory=8454144 -C link-arg=--max-memory=16777216 -C link-arg=--export=__stack_pointer -C link-arg=--export=__data_end -C link-arg=--export=__heap_base

include vendor/toko-tooling/make/consumer.mk
include vendor/toko-tooling/make/cargo-dependencies.mk

# Ordinary checks must never install compilers or fetch dependencies.
export RUSTUP_AUTO_INSTALL := 0
check lint doc-check ci deps-check deps-pins deps-boundaries deps-tree: export CARGO_NET_OFFLINE := true
m0-test m0-lint-wasm: export CARGO_NET_OFFLINE := true

.PHONY: m0-test m0-lint-wasm
m0-test m0-lint-wasm: | toko-execution-check
retention-test retention-lint retention-tools-check: export CARGO_NET_OFFLINE := true

.PHONY: retention-setup retention-tools-check retention-lint retention-test
retention-setup retention-tools-check retention-lint retention-test: | toko-execution-check

.PHONY: rust-startup-build rust-startup-lint rust-startup-test
rust-startup-build rust-startup-lint rust-startup-test: export CARGO_NET_OFFLINE := true
rust-startup-build rust-startup-lint rust-startup-test: | toko-execution-check

.PHONY: managed-graph-build managed-graph-lint managed-graph-test
managed-graph-build managed-graph-lint managed-graph-test: export CARGO_NET_OFFLINE := true
managed-graph-build managed-graph-lint managed-graph-test: | toko-execution-check

.PHONY: benchmark-simple benchmark-simple-build benchmark-simple-lint
benchmark-simple benchmark-simple-build benchmark-simple-lint: export CARGO_NET_OFFLINE := true
benchmark-simple benchmark-simple-build benchmark-simple-lint: | toko-execution-check

.PHONY: native-feasibility-test native-feasibility-lint native-layout-probe
native-feasibility-test native-feasibility-lint native-layout-probe: export CARGO_NET_OFFLINE := true
native-feasibility-test native-feasibility-lint native-layout-probe: | toko-execution-check

.PHONY: native-provider-probe native-provider-test
native-provider-probe native-provider-test: export CARGO_NET_OFFLINE := true
native-provider-probe native-provider-test: | toko-execution-check

.PHONY: checkpoint-provider-build checkpoint-provider-lint checkpoint-provider-test
checkpoint-provider-build checkpoint-provider-lint checkpoint-provider-test: export CARGO_NET_OFFLINE := true
checkpoint-provider-build checkpoint-provider-lint checkpoint-provider-test: | toko-execution-check

.PHONY: help check lint doc-check deps-check deps-pins deps-boundaries deps-tree
check lint doc-check deps-check deps-pins deps-boundaries deps-tree: | toko-execution-check

help:
	@printf '%s\n' \
	  'make install-tools / tools-check  Prepare / admit common Toko tools' \
	  'make check                       Check the research library (locked, offline)' \
	  'make fmt / fmt-check             Format / check workspace formatting' \
	  'make lint / doc-check            Strict Clippy / Rustdoc for the library' \
	  'make m0-test                     Run native M0 image and capability tests' \
	  'make m0-lint-wasm                Check the no_std image library on wasm32' \
	  'make retention-setup             Explicitly prepare the retention-test server' \
	  'make retention-tools-check       Admit the prepared server offline' \
	  'make retention-lint              Strict Clippy for the Rust test runner' \
	  'make retention-test              Run installed Wasm upgrade/rollback test' \
	  'make rust-startup-build           Build the controlled Rust A/B fixtures' \
	  'make rust-startup-lint            Lint Rust fixtures and artifact admission' \
	  'make rust-startup-test            Admit artifacts and test installed startup' \
	  'make managed-graph-build          Build managed graph A/B fixtures' \
	  'make managed-graph-lint           Lint the graph kernel and fixtures' \
	  'make managed-graph-test           Test installed graph upgrades and rollback' \
	  'make benchmark-simple            Build and measure M0 vs published Storage' \
	  'make benchmark-simple-lint       Lint both benchmark backends and runner' \
	  'make native-feasibility-test     Test host-only scoped ownership boundaries' \
	  'make native-feasibility-lint     Lint the native ownership experiment' \
	  'make native-layout-probe         Inspect host/wasm compiler layout diagnostics' \
	  'make native-provider-probe       Compile and inspect provider linkage; no execution' \
	  'make native-provider-test        Test installed scalar access, custody and rollback' \
	  'make checkpoint-provider-build  Build ordinary-upgrade checkpoint fixtures' \
	  'make checkpoint-provider-lint   Lint provider fixtures and installed test' \
	  'make checkpoint-provider-test   Test fresh-heap recovery and import rollback' \
	  'make ci                          Aggregate hosted validation gate' \
	  'make install-hooks               Activate the canonical formatting hook' \
	  'make deps-check                  Aggregate dependency policy checks' \
	  'make deps-pins                   Check declarations and root inheritance' \
	  'make deps-boundaries             Check IC boundaries and tooling exclusion' \
	  'make deps-list                   List local packages, versions and groups' \
	  'make deps-tree                   Inspect the locked dependency graph' \
	  'make deps-update                 Explicit online Cargo.lock refresh' \
	  'make toko-tooling-check          Admit pin, lockfile and snapshot bytes'

check:
	+cargo check -p ic-storage-new --locked --offline

lint:
	+cargo clippy -p ic-storage-new --all-targets --locked --offline -- -D warnings

doc-check: export RUSTDOCFLAGS := -D warnings
doc-check:
	+cargo doc -p ic-storage-new --no-deps --locked --offline
	+cargo doc $(RUST_STARTUP_CARGO) --target-dir target/qualification --no-deps
	+cargo doc $(RUST_STARTUP_CARGO) --target-dir target/qualification --features managed-graph --no-deps

# Host setup and locked dependency fetching happen explicitly before this gate.
ci: fmt-check deps-check lint doc-check m0-test m0-lint-wasm retention-lint rust-startup-lint managed-graph-lint benchmark-simple-lint native-feasibility-test native-layout-probe native-provider-probe checkpoint-provider-lint

deps-check: toko-tooling-check deps-pins deps-boundaries

deps-pins:
	+bash "$(TOKO_TOOLING_ROOT)/scripts/ci/check-dependency-pins.sh" --consumer "$(CURDIR)" --cargo-inheritance

deps-boundaries:
	+bash "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-dependency-boundaries.sh" --consumer "$(CURDIR)" --external-ic
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)" --package ic-storage-new --target all --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-retention-probe --target all --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-native-feasibility --target all --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-checkpoint-provider-probe --target wasm32-unknown-unknown --all-features --require ic-storage=1 --require ic-platform-runtime=1 --forbid ic-platform --forbid ic-platform-types --forbid ic-platform-runtime-guard --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-rust-startup-probe --target wasm32-unknown-unknown --require ic-platform-runtime=1 --forbid ic-platform --forbid ic-platform-types --forbid ic-platform-runtime-guard --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-rust-startup-probe --target wasm32-unknown-unknown --all-features --require ic-storage-new=1 --require ic-platform-runtime=1 --forbid ic-platform --forbid ic-platform-types --forbid ic-platform-runtime-guard --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-simple-benchmarks --target wasm32-unknown-unknown --all-features --require ic-storage=1 --require ic-platform-runtime=1 --forbid ic-platform --forbid ic-platform-types --forbid ic-platform-runtime-guard --forbid toko-tooling

deps-tree:
	+cargo tree -p ic-storage-new --locked --offline

m0-test:
	+cargo test -p ic-storage-new --lib --locked --offline m0::
	+cargo test -p ic-storage-new --doc --locked --offline

m0-lint-wasm:
	+cargo clippy -p ic-storage-new --lib --target wasm32-unknown-unknown --locked --offline -- -D warnings

retention-setup:
	+@server="$$($(RETENTION_TOOL))" && "$$server" setup

retention-tools-check:
	+@server="$$($(RETENTION_TOOL) --check)" && "$$server" check

retention-lint:
	+cargo clippy $(RETENTION_CARGO) --test retention -- -D warnings

retention-test:
	+@server="$$($(RETENTION_TOOL) --check)" && "$$server" run -- cargo test $(RETENTION_CARGO) --test retention -- --nocapture

# This experimental linker recipe is deliberately separate from production
# optimization. Final artifact admission rejects a conflicting layout.
rust-startup-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Rust startup probe requires no environment flag/linker overrides' >&2; exit 1; }
	+cargo rustc $(RUST_STARTUP_CARGO) --target-dir target/qualification/rust-a -- $(RUST_STARTUP_LINK)
	+cargo rustc $(RUST_STARTUP_CARGO) --target-dir target/qualification/rust-b --features version-b -- $(RUST_STARTUP_LINK)

rust-startup-lint:
	+cargo clippy $(RUST_STARTUP_CARGO) --target-dir target/qualification -- -D warnings
	+cargo clippy $(RUST_STARTUP_CARGO) --target-dir target/qualification --features version-b -- -D warnings
	+cargo clippy $(RETENTION_CARGO) --test rust_startup -- -D warnings

rust-startup-test:
	+@server="$$($(RETENTION_TOOL) --check)" && "$$server" run -- cargo test $(RETENTION_CARGO) --test rust_startup -- --skip graph_tests --nocapture

managed-graph-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Managed graph probe requires no environment flag/linker overrides' >&2; exit 1; }
	+cargo rustc $(RUST_STARTUP_CARGO) --target-dir target/qualification/graph-a --features managed-graph -- $(RUST_STARTUP_LINK)
	+cargo rustc $(RUST_STARTUP_CARGO) --target-dir target/qualification/graph-b --features managed-graph,version-b -- $(RUST_STARTUP_LINK)

managed-graph-lint:
	+cargo clippy $(RUST_STARTUP_CARGO) --target-dir target/qualification --features managed-graph -- -D warnings
	+cargo clippy $(RUST_STARTUP_CARGO) --target-dir target/qualification --features managed-graph,version-b -- -D warnings

managed-graph-test:
	+@server="$$($(RETENTION_TOOL) --check)" && "$$server" run -- cargo test $(RETENTION_CARGO) --test rust_startup graph_tests -- --nocapture

# Opt-in measurements, not a timing threshold or release gate.
benchmark-simple-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Benchmarks require no environment flag/linker overrides' >&2; exit 1; }
	+cargo rustc $(RUST_STARTUP_CARGO) --target-dir target/qualification/bench-managed --features managed-bench -- $(RUST_STARTUP_LINK)
	+cargo rustc $(BENCH_CARGO) --example storage-wasm --features storage --target wasm32-unknown-unknown --release --target-dir target/qualification/bench-storage -- $(RUST_STARTUP_LINK)

benchmark-simple-lint:
	+cargo clippy $(BENCH_CARGO) --example compare --target-dir target/qualification -- -D warnings
	+cargo clippy $(BENCH_CARGO) --example storage-wasm --features storage --target wasm32-unknown-unknown --release --target-dir target/qualification -- -D warnings
	+cargo clippy $(RUST_STARTUP_CARGO) --features managed-bench --target-dir target/qualification -- -D warnings

benchmark-simple: benchmark-simple-build
	+@server="$$($(RETENTION_TOOL) --check)" && "$$server" run -- cargo run $(BENCH_CARGO) --example compare --target-dir target/qualification

native-feasibility-lint:
	+cargo clippy $(NATIVE_FEASIBILITY_CARGO) --all-targets -- -D warnings
	+cargo clippy $(NATIVE_FEASIBILITY_CARGO) --example layout_probe --features layout-swapped -- -D warnings

# Keep lint ordered before execution even when Make runs in parallel.
native-feasibility-test: native-feasibility-lint
	+cargo test $(NATIVE_FEASIBILITY_CARGO)

native-layout-probe: native-feasibility-lint
	+bash qualification/ic-storage-native-feasibility/scripts/run-layout-probe.sh

native-provider-probe:
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+bash qualification/ic-storage-native-feasibility/scripts/run-provider-probe.sh

native-provider-test:
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+@server="$$($(RETENTION_TOOL) --check)" && "$$server" run -- bash qualification/ic-storage-native-feasibility/scripts/run-provider-probe.sh installed

checkpoint-provider-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Checkpoint fixtures require no environment flag/linker overrides' >&2; exit 1; }
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-a -- $(CHECKPOINT_LINK)
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-b --features version-b -- $(CHECKPOINT_LINK)

checkpoint-provider-lint:
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification -- -D warnings
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features version-b -- -D warnings
	+cargo clippy $(RETENTION_CARGO) --test checkpoint_provider -- -D warnings

checkpoint-provider-test:
	+@server="$$($(RETENTION_TOOL) --check)" && "$$server" run -- cargo test $(RETENTION_CARGO) --test checkpoint_provider -- --nocapture
