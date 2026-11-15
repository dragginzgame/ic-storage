.DEFAULT_GOAL := help

include make/consumer.mk

# Ordinary checks must never install compilers or fetch dependencies.
export RUSTUP_AUTO_INSTALL := 0
check deps-check deps-pins deps-boundaries deps-tree: export CARGO_NET_OFFLINE := true

.PHONY: help check deps-check deps-pins deps-boundaries deps-tree deps-update
check deps-check deps-pins deps-boundaries deps-tree deps-update: | toko-execution-check

help:
	@printf '%s\n' \
	  'make install-tools / tools-check  Prepare / admit common Toko tools' \
	  'make check                       Check the research library (locked, offline)' \
	  'make fmt / fmt-check             Format / check workspace formatting' \
	  'make deps-check                  Aggregate dependency policy checks' \
	  'make deps-pins                   Check declarations and root inheritance' \
	  'make deps-boundaries             Check IC boundaries and tooling exclusion' \
	  'make deps-tree                   Inspect the locked dependency graph' \
	  'make deps-update                 Explicit online Cargo.lock refresh' \
	  'make toko-tooling-check          Admit pin, lockfile and snapshot bytes'

check:
	+cargo check -p ic-storage-new --locked --offline

deps-check: toko-tooling-check deps-pins deps-boundaries

deps-pins:
	+bash scripts/ci/check-dependency-pins.sh --consumer "$(CURDIR)" --cargo-inheritance

deps-boundaries:
	+bash scripts/ci/check-cargo-dependency-boundaries.sh --consumer "$(CURDIR)" --external-ic
	+perl scripts/ci/check-cargo-graph.pl --consumer "$(CURDIR)" --package ic-storage-new --target all --forbid toko-tooling

deps-tree:
	+cargo tree -p ic-storage-new --locked --offline

# Exact tooling selection remains in Cargo.toml; updating the lock never installs it.
deps-update:
	+cargo update --verbose
