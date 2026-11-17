.DEFAULT_GOAL := help
FORMAT_RUST_ARGS := --extra-workspace qualification
CARGO_PUBLISH_PACKAGES := ic-storage-derive ic-storage
CARGO_PUBLISH_REGISTRY := crates-io
CARGO_PUBLISH_ALL_FEATURES := 1
CARGO_PUBLISH_VERIFY_TARGETS := deps-pins native-core-test

RETENTION_CARGO := --manifest-path qualification/Cargo.toml -p ic-storage-retention-probe --target-dir target/qualification --locked --offline
BENCH_CARGO := --manifest-path qualification/Cargo.toml -p ic-storage-simple-benchmarks --locked --offline
NATIVE_FEASIBILITY_CARGO := --manifest-path qualification/Cargo.toml -p ic-storage-native-feasibility --target-dir target/qualification --locked --offline
CHECKPOINT_CARGO := --manifest-path qualification/Cargo.toml -p ic-storage-checkpoint-provider-probe --target wasm32-unknown-unknown --locked --offline --release
CHECKPOINT_LINK := -C link-arg=--stack-first -C link-arg=-zstack-size=1048576 -C link-arg=--initial-memory=2097152 -C link-arg=--max-memory=16777216
NATIVE_CHECKPOINT_LINK := -C link-arg=--stack-first -C link-arg=-zstack-size=65536 -C link-arg=--global-base=196608 -C link-arg=--initial-memory=2097152 -C link-arg=--max-memory=16777216 -C link-arg=--export=__stack_pointer -C link-arg=--export=__data_end -C link-arg=--export=__heap_base
# Keep the separately prepared launcher outside the consumer Cargo graph;
# installed tests require its explicitly owned server URL.
RETENTION_TOOL := bash vendor/toko-tooling/scripts/dev/install-rust-tools.sh --consumer "$(CURDIR)" --package ic-testkit --version 0.109.0 --bin ic-testkit-server --profile release --features cli
RETENTION_DIRECTORY := $(CURDIR)/.tools/testkit-server
RETENTION_EVIDENCE := $(CURDIR)/target/evidence/testkit
define retention-run
umask 077; server="$$($(RETENTION_TOOL) --check)" && mkdir -p "$(RETENTION_EVIDENCE)" && \
attempt="$$(mktemp -d "$(RETENTION_EVIDENCE)/run.XXXXXX")" && \
"$$server" run "$(RETENTION_DIRECTORY)" "$$attempt/evidence" --ttl 300 -- $(1)
endef
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
.PHONY: native-core-test native-map-firehose native-map-pool-firehose
native-core-test native-map-firehose native-map-pool-firehose: export CARGO_NET_OFFLINE := true
native-core-test native-map-firehose native-map-pool-firehose: | toko-execution-check
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
.PHONY: benchmark-map benchmark-map-build
.PHONY: benchmark-matrix benchmark-matrix-smoke benchmark-matrix-build
benchmark-matrix benchmark-matrix-smoke benchmark-matrix-build: export CARGO_NET_OFFLINE := true
benchmark-matrix benchmark-matrix-smoke benchmark-matrix-build: | toko-execution-check
benchmark-map benchmark-map-build: export CARGO_NET_OFFLINE := true
benchmark-map benchmark-map-build: | toko-execution-check
benchmark-simple benchmark-simple-build benchmark-simple-lint: export CARGO_NET_OFFLINE := true
benchmark-simple benchmark-simple-build benchmark-simple-lint: | toko-execution-check

.PHONY: native-feasibility-test native-feasibility-lint native-layout-probe
native-feasibility-test native-feasibility-lint native-layout-probe: export CARGO_NET_OFFLINE := true
native-feasibility-test native-feasibility-lint native-layout-probe: | toko-execution-check

.PHONY: native-provider-probe native-provider-test
.PHONY: native-map-probe native-map-test
.PHONY: native-session-probe native-session-test
native-session-probe native-session-test: export CARGO_NET_OFFLINE := true
native-session-probe native-session-test: | toko-execution-check
.PHONY: native-map-checkpoint-build native-map-checkpoint-test
.PHONY: native-map-job-build native-map-job-test
.PHONY: native-map-paged-build native-map-paged-test
native-map-paged-build native-map-paged-test: export CARGO_NET_OFFLINE := true
native-map-paged-build native-map-paged-test: | toko-execution-check
.PHONY: native-map-page-test
.PHONY: paged-store-build paged-store-test
paged-store-build paged-store-test: export CARGO_NET_OFFLINE := true
paged-store-build paged-store-test: | toko-execution-check
native-map-page-test: export CARGO_NET_OFFLINE := true
native-map-page-test: | toko-execution-check
native-map-job-build native-map-job-test: export CARGO_NET_OFFLINE := true
native-map-job-build native-map-job-test: | toko-execution-check
native-map-probe native-map-test native-map-checkpoint-build native-map-checkpoint-test: export CARGO_NET_OFFLINE := true
native-map-probe native-map-test native-map-checkpoint-build native-map-checkpoint-test: | toko-execution-check
.PHONY: native-checkpoint-build native-checkpoint-test
.PHONY: native-record-build native-record-test
.PHONY: native-record-vector-build native-record-vector-test
native-record-vector-build native-record-vector-test: export CARGO_NET_OFFLINE := true
native-record-vector-build native-record-vector-test: | toko-execution-check
native-record-build native-record-test: export CARGO_NET_OFFLINE := true
native-record-build native-record-test: | toko-execution-check
.PHONY: native-job-build native-job-test
native-job-build native-job-test: export CARGO_NET_OFFLINE := true
native-job-build native-job-test: | toko-execution-check

.PHONY: native-record-job-build native-record-job-test
native-record-job-build native-record-job-test: export CARGO_NET_OFFLINE := true
native-record-job-build native-record-job-test: | toko-execution-check
native-checkpoint-build native-checkpoint-test: export CARGO_NET_OFFLINE := true
native-checkpoint-build native-checkpoint-test: | toko-execution-check
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
	  'make native-core-test           Test scoped native operations and Store declarations' \
	  'make native-map-firehose        Exercise one million keyed records through the public API' \
	  'make native-map-pool-firehose   Grow and reopen one million records in a shared host pool' \
	  'make native-map-test            Qualify installed map upgrades, custody and rollback' \
	  'make native-session-test        Qualify map owners reused between messages' \
	  'make native-map-checkpoint-test Recover two maps through the governed checkpoint provider' \
	  'make native-map-job-test        Qualify durable two-map recovery jobs' \
	  'make native-map-paged-test      Qualify paged recovery into retained maps' \
	  'make paged-store-test           Qualify installed paged storage transactions' \
	  'make native-map-page-test       Qualify paged checkpoints on governed host storage' \
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
	  'make benchmark-map               Compare native map with ic-stable-structures' \
	  'make benchmark-matrix            Full generation-owned map / stable-structures matrix' \
	  'make benchmark-matrix-smoke      One paired matrix scenario' \
	  'make benchmark-simple-lint       Lint both benchmark backends and runner' \
	  'make native-feasibility-test     Test host-only scoped ownership boundaries' \
	  'make native-feasibility-lint     Lint the native ownership experiment' \
	  'make native-layout-probe         Inspect host/wasm compiler layout diagnostics' \
	  'make native-provider-probe       Compile and inspect provider linkage; no execution' \
	  'make native-provider-test        Test installed scalar/vector custody and rollback' \
	  'make checkpoint-provider-build  Build ordinary-upgrade checkpoint fixtures' \
	  'make checkpoint-provider-lint   Lint provider fixtures and installed test' \
	  'make checkpoint-provider-test   Test fresh-heap recovery and import rollback' \
	  'make native-checkpoint-test     Test native export and incompatible-domain recovery' \
	  'make native-record-test         Test installed record conversion and recovery' \
	  'make native-record-vector-test  Test retained record conversion and recovery' \
	  'make native-record-job-test     Test governed record-vector migration' \
	  'make native-job-test            Test controller authority and single-use migration' \
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
	+cargo check --workspace --locked --offline

lint:
	+cargo clippy --workspace --all-targets --locked --offline -- -D warnings

doc-check: export RUSTDOCFLAGS := -D warnings
doc-check:
	+cargo doc --workspace --no-deps --locked --offline
	+cargo doc $(RUST_STARTUP_CARGO) --target-dir target/qualification --no-deps
	+cargo doc $(RUST_STARTUP_CARGO) --target-dir target/qualification --features managed-graph --no-deps

# Host setup and locked dependency fetching happen explicitly before this gate.
ci: fmt-check deps-check lint doc-check m0-test m0-lint-wasm native-core-test native-map-page-test retention-lint rust-startup-lint managed-graph-lint benchmark-simple-lint native-feasibility-test native-layout-probe native-provider-probe native-map-probe native-session-probe checkpoint-provider-lint

deps-check: toko-tooling-check deps-pins deps-boundaries

deps-pins:
	+bash "$(TOKO_TOOLING_ROOT)/scripts/ci/check-dependency-pins.sh" --consumer "$(CURDIR)" --cargo-inheritance

# The frozen Storage 0.74.1 oracle retains Runtime 0.22.5/ic0; our callers use
# 0.23.13/system-raw (requirement 0.23.12). Neither selects an executor or shares Runtime Rust types.
# Admit both only for oracle-bearing fixtures; startup still requires one.
deps-boundaries:
	+bash "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-dependency-boundaries.sh" --consumer "$(CURDIR)" --external-ic
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-testkit --target all --require pocket-ic=1 --forbid ic-host-artifacts --forbid ic-host-fs --forbid ic-host-process --forbid ic-platform --forbid ic-platform-types --forbid ic-platform-protocol --forbid toko-tooling --forbid-feature ic-testkit/held-reply-fixtures
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)" --package ic-storage --target all --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)" --package ic-storage-m0 --target all --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-retention-probe --target all --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-native-feasibility --target all --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-checkpoint-provider-probe --target wasm32-unknown-unknown --all-features --require ic-storage=2 --require ic-platform-runtime=2 --forbid ic-platform --forbid ic-platform-types --forbid ic-platform-runtime-guard --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-rust-startup-probe --target wasm32-unknown-unknown --require ic-platform-runtime=1 --forbid ic-platform --forbid ic-platform-types --forbid ic-platform-runtime-guard --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-rust-startup-probe --target wasm32-unknown-unknown --all-features --require ic-storage-m0=1 --require ic-platform-runtime=1 --forbid ic-platform --forbid ic-platform-types --forbid ic-platform-runtime-guard --forbid toko-tooling
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)/qualification" --package ic-storage-simple-benchmarks --target wasm32-unknown-unknown --all-features --require ic-storage=1 --require ic-platform-runtime=2 --forbid ic-platform --forbid ic-platform-types --forbid ic-platform-runtime-guard --forbid toko-tooling

deps-tree:
	+cargo tree -p ic-storage --locked --offline

m0-test:
	+cargo test -p ic-storage-m0 --lib --locked --offline m0::
	+cargo test -p ic-storage-m0 --doc --locked --offline

m0-lint-wasm:
	+cargo clippy -p ic-storage-m0 --lib --target wasm32-unknown-unknown --locked --offline -- -D warnings

native-core-test:
	+cargo test -p ic-storage --locked --offline
	+cargo test -p ic-storage --features derive,map --locked --offline
	+cargo clippy -p ic-storage --all-targets --features derive,map --locked --offline -- -D warnings
	+cargo clippy -p ic-storage --lib --target wasm32-unknown-unknown --locked --offline -- -D warnings
	+cargo clippy -p ic-storage --lib --features derive,map --target wasm32-unknown-unknown --locked --offline -- -D warnings
	+RUSTDOCFLAGS='-D warnings' cargo doc -p ic-storage --features derive,map --no-deps --locked --offline

native-map-firehose:
	+cargo test -p ic-storage --features map --release --test map_firehose --locked --offline -- --ignored --nocapture

native-map-pool-firehose:
	+cargo test $(NATIVE_FEASIBILITY_CARGO) --release --test provider_region_tests million_records_share_a_pool_with_other_roots -- --ignored --nocapture

native-map-probe:
	rustfmt --edition 2024 --check qualification/ic-storage-native-feasibility/tests/fixtures/provider_map.rs
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+bash qualification/ic-storage-native-feasibility/scripts/run-map-probe.sh

native-map-test:
	rustfmt --edition 2024 --check qualification/ic-storage-native-feasibility/tests/fixtures/provider_map.rs
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-map-probe.sh installed)

native-map-page-test:
	+cargo clippy --manifest-path qualification/Cargo.toml -p ic-storage-checkpoint-provider-probe --target-dir target/qualification --features native-map-job --lib --tests --locked --offline -- -D warnings
	+cargo test --manifest-path qualification/Cargo.toml -p ic-storage-checkpoint-provider-probe --target-dir target/qualification --features native-map-job --lib provider::paged_tests --locked --offline -- --nocapture

paged-store-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Paged store probe requires the pinned compiler without overrides' >&2; exit 1; }
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features paged-store -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/paged-store-a --features paged-store -- $(CHECKPOINT_LINK)
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/paged-store-b --features paged-store,version-b -- $(CHECKPOINT_LINK)

paged-store-test: paged-store-build
	+cargo clippy $(RETENTION_CARGO) --test paged_store -- -D warnings
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-paged-store-probe.sh)

native-session-probe:
	rustfmt --edition 2024 --check qualification/ic-storage-native-feasibility/tests/fixtures/provider_session.rs
	+cargo clippy $(RETENTION_CARGO) --test rust_startup -- -D warnings
	+bash qualification/ic-storage-native-feasibility/scripts/run-session-probe.sh

native-session-test:
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-session-probe.sh installed)

native-map-paged-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Paged native probe requires the pinned compiler without overrides' >&2; exit 1; }
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-map-paged -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-map-paged --features native-map-paged -- $(subst --global-base=196608,--global-base=655360,$(NATIVE_CHECKPOINT_LINK))
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features allocation-metrics -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-map-paged-allocations --features allocation-metrics -- $(subst --global-base=196608,--global-base=655360,$(NATIVE_CHECKPOINT_LINK))

native-map-paged-test: native-map-paged-build
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-map-probe.sh paged)

native-map-job-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Map job probe requires the pinned compiler without overrides' >&2; exit 1; }
	+cargo test $(NATIVE_FEASIBILITY_CARGO) --lib migration_job
	+cargo test --manifest-path qualification/Cargo.toml -p ic-storage-checkpoint-provider-probe --target-dir target/qualification --features native-map-job --lib provider::map_job::tests --locked --offline
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-map-job -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-map-job --features native-map-job -- $(NATIVE_CHECKPOINT_LINK)

native-map-job-test: native-map-job-build
	rustfmt --edition 2024 --check qualification/ic-storage-native-feasibility/tests/fixtures/provider_map.rs
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-map-probe.sh job)

native-map-checkpoint-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Map checkpoint probe requires the pinned compiler without overrides' >&2; exit 1; }
	+cargo test $(NATIVE_FEASIBILITY_CARGO) --lib checkpoint::maps
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-map -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-map --features native-map -- $(NATIVE_CHECKPOINT_LINK)

native-map-checkpoint-test: native-map-checkpoint-build
	rustfmt --edition 2024 --check qualification/ic-storage-native-feasibility/tests/fixtures/provider_map.rs
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-map-probe.sh recovery)

retention-setup:
	+@umask 077; server="$$($(RETENTION_TOOL))" && mkdir -p "$(RETENTION_EVIDENCE)" && \
	  attempt="$$(mktemp -d "$(RETENTION_EVIDENCE)/setup.XXXXXX")" && \
	  "$$server" setup "$(RETENTION_DIRECTORY)" "$$attempt/evidence" /usr/bin/curl /usr/bin/gzip

retention-tools-check:
	+@server="$$($(RETENTION_TOOL) --check)" && "$$server" check "$(RETENTION_DIRECTORY)"

retention-lint:
	+cargo test $(RETENTION_CARGO) --lib
	+cargo clippy $(RETENTION_CARGO) --lib -- -D warnings
	+cargo clippy $(RETENTION_CARGO) --test retention -- -D warnings

retention-test:
	+@$(call retention-run,cargo test $(RETENTION_CARGO) --test retention -- --nocapture)

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
	+@$(call retention-run,cargo test $(RETENTION_CARGO) --test rust_startup -- --skip graph_tests --nocapture)

managed-graph-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Managed graph probe requires no environment flag/linker overrides' >&2; exit 1; }
	+cargo rustc $(RUST_STARTUP_CARGO) --target-dir target/qualification/graph-a --features managed-graph -- $(RUST_STARTUP_LINK)
	+cargo rustc $(RUST_STARTUP_CARGO) --target-dir target/qualification/graph-b --features managed-graph,version-b -- $(RUST_STARTUP_LINK)

managed-graph-lint:
	+cargo clippy $(RUST_STARTUP_CARGO) --target-dir target/qualification --features managed-graph -- -D warnings
	+cargo clippy $(RUST_STARTUP_CARGO) --target-dir target/qualification --features managed-graph,version-b -- -D warnings

managed-graph-test:
	+@$(call retention-run,cargo test $(RETENTION_CARGO) --test rust_startup graph_tests -- --nocapture)

# Opt-in measurements, not a timing threshold or release gate.
benchmark-simple-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Benchmarks require no environment flag/linker overrides' >&2; exit 1; }
	+cargo rustc $(RUST_STARTUP_CARGO) --target-dir target/qualification/bench-managed --features managed-bench -- $(RUST_STARTUP_LINK)
	+cargo rustc $(BENCH_CARGO) --example storage-wasm --features storage --target wasm32-unknown-unknown --release --target-dir target/qualification/bench-storage -- $(RUST_STARTUP_LINK)

benchmark-map-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Benchmarks require no environment flag/linker overrides' >&2; exit 1; }
	+cargo clippy $(BENCH_CARGO) --example stable-map-wasm --features stable-map --target wasm32-unknown-unknown --release --target-dir target/qualification -- -D warnings
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+cargo rustc $(BENCH_CARGO) --example stable-map-wasm --features stable-map --target wasm32-unknown-unknown --release --target-dir target/qualification/bench-stable-map -- $(CHECKPOINT_LINK)

benchmark-map: benchmark-map-build
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-map-probe.sh comparison)

# Explicitly on demand; benchmark populations are not a routine CI/release gate.
benchmark-matrix-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Matrix requires no environment flag/linker overrides' >&2; exit 1; }
	+cargo clippy $(BENCH_CARGO) --example stable-matrix-wasm --features stable-map --target wasm32-unknown-unknown --release --target-dir target/qualification -- -D warnings
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+cargo rustc $(BENCH_CARGO) --example stable-matrix-wasm --features stable-map --target wasm32-unknown-unknown --release --target-dir target/qualification/bench-stable-matrix -- $(CHECKPOINT_LINK)

benchmark-matrix: benchmark-matrix-build
	+@$(call retention-run,env STORAGE_MATRIX_PROFILE=full bash qualification/ic-storage-native-feasibility/scripts/run-session-probe.sh matrix)

benchmark-matrix-smoke: benchmark-matrix-build
	+@$(call retention-run,env STORAGE_MATRIX_PROFILE=smoke bash qualification/ic-storage-native-feasibility/scripts/run-session-probe.sh matrix)

benchmark-simple-lint:
	+cargo clippy $(BENCH_CARGO) --example compare --target-dir target/qualification -- -D warnings
	+cargo clippy $(BENCH_CARGO) --example storage-wasm --features storage --target wasm32-unknown-unknown --release --target-dir target/qualification -- -D warnings
	+cargo clippy $(RUST_STARTUP_CARGO) --features managed-bench --target-dir target/qualification -- -D warnings

benchmark-simple: benchmark-simple-build
	+@$(call retention-run,cargo run $(BENCH_CARGO) --example compare --target-dir target/qualification)

native-feasibility-lint:
	rustfmt --edition 2024 --check qualification/ic-storage-native-feasibility/tests/fixtures/provider_vector.rs qualification/ic-storage-native-feasibility/tests/fixtures/provider_record_vector.rs
	+cargo clippy -p ic-storage-derive --all-targets --locked --offline -- -D warnings
	+cargo clippy $(NATIVE_FEASIBILITY_CARGO) --all-targets -- -D warnings
	+cargo clippy $(NATIVE_FEASIBILITY_CARGO) --example layout_probe --features layout-swapped -- -D warnings

# Keep lint ordered before execution even when Make runs in parallel.
native-feasibility-test: native-feasibility-lint
	+cargo test -p ic-storage-derive --locked --offline
	+cargo test $(NATIVE_FEASIBILITY_CARGO)

native-layout-probe: native-feasibility-lint
	+bash qualification/ic-storage-native-feasibility/scripts/run-layout-probe.sh

native-provider-probe:
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+bash qualification/ic-storage-native-feasibility/scripts/run-provider-probe.sh

native-provider-test:
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-provider-probe.sh installed)

checkpoint-provider-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Checkpoint fixtures require no environment flag/linker overrides' >&2; exit 1; }
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-a -- $(CHECKPOINT_LINK)
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-b --features version-b -- $(CHECKPOINT_LINK)

checkpoint-provider-lint:
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-bridge -- -D warnings
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification -- -D warnings
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features version-b -- -D warnings
	+cargo clippy $(RETENTION_CARGO) --test checkpoint_provider -- -D warnings

checkpoint-provider-test:
	+@$(call retention-run,cargo test $(RETENTION_CARGO) --test checkpoint_provider -- --nocapture)

native-checkpoint-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Native checkpoint probe requires no environment flag/linker overrides' >&2; exit 1; }
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-bridge -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-native --features native-bridge -- $(NATIVE_CHECKPOINT_LINK)

native-checkpoint-test: native-checkpoint-build
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-provider-probe.sh migration)

native-record-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Native checkpoint probe requires no environment flag/linker overrides' >&2; exit 1; }
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-record -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-record --features native-record -- $(NATIVE_CHECKPOINT_LINK)

native-record-test: native-record-build
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-provider-probe.sh record)

native-record-vector-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Native checkpoint probe requires no environment flag/linker overrides' >&2; exit 1; }
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-record-vector -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-record-vector --features native-record-vector -- $(NATIVE_CHECKPOINT_LINK)
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-record-vector-source -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-record-vector-source --features native-record-vector-source -- $(subst --global-base=196608,--global-base=65536,$(NATIVE_CHECKPOINT_LINK))
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-record-vector-v2 -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-record-vector-v2 --features native-record-vector-v2 -- $(NATIVE_CHECKPOINT_LINK)
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-record-vector-convert -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-record-vector-convert --features native-record-vector-convert -- $(NATIVE_CHECKPOINT_LINK)

native-record-vector-test: native-record-vector-build
	+cargo test $(NATIVE_FEASIBILITY_CARGO) --lib checkpoint::record_vector
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-provider-probe.sh record-vector)

native-job-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Native job probe requires no environment flag/linker overrides' >&2; exit 1; }
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-job -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-job --features native-job -- $(NATIVE_CHECKPOINT_LINK)

native-job-test: native-job-build
	+cargo test $(NATIVE_FEASIBILITY_CARGO) --lib migration_job
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-provider-probe.sh job)

native-record-job-build:
	@test -z "$${RUSTFLAGS}$${CARGO_ENCODED_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS}$${CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER}" || { echo 'Native job probe requires no environment flag/linker overrides' >&2; exit 1; }
	+cargo clippy $(CHECKPOINT_CARGO) --target-dir target/qualification --features native-record-vector-job -- -D warnings
	+cargo rustc $(CHECKPOINT_CARGO) --target-dir target/qualification/checkpoint-record-vector-job --features native-record-vector-job -- $(NATIVE_CHECKPOINT_LINK)

native-record-job-test: native-record-job-build
	+cargo test $(NATIVE_FEASIBILITY_CARGO) --lib migration_job
	+cargo clippy $(RETENTION_CARGO) --test provider_link -- -D warnings
	+@$(call retention-run,bash qualification/ic-storage-native-feasibility/scripts/run-provider-probe.sh record-job)
