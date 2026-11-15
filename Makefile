.DEFAULT_GOAL := help

include vendor/toko-tooling/make/consumer.mk
include vendor/toko-tooling/make/cargo-dependencies.mk

# Ordinary checks must never install compilers or fetch dependencies.
export RUSTUP_AUTO_INSTALL := 0
check lint doc-check ci deps-check deps-pins deps-boundaries deps-tree: export CARGO_NET_OFFLINE := true

.PHONY: help check lint doc-check deps-check deps-pins deps-boundaries deps-tree
check lint doc-check deps-check deps-pins deps-boundaries deps-tree: | toko-execution-check

help:
	@printf '%s\n' \
	  'make install-tools / tools-check  Prepare / admit common Toko tools' \
	  'make check                       Check the research library (locked, offline)' \
	  'make fmt / fmt-check             Format / check workspace formatting' \
	  'make lint / doc-check            Strict Clippy / Rustdoc for the library' \
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

# Host setup and locked dependency fetching happen explicitly before this gate.
ci: fmt-check deps-check lint doc-check

deps-check: toko-tooling-check deps-pins deps-boundaries

deps-pins:
	+bash "$(TOKO_TOOLING_ROOT)/scripts/ci/check-dependency-pins.sh" --consumer "$(CURDIR)" --cargo-inheritance

deps-boundaries:
	+bash "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-dependency-boundaries.sh" --consumer "$(CURDIR)" --external-ic
	+perl "$(TOKO_TOOLING_ROOT)/scripts/ci/check-cargo-graph.pl" --consumer "$(CURDIR)" --package ic-storage-new --target all --forbid toko-tooling

deps-tree:
	+cargo tree -p ic-storage-new --locked --offline
