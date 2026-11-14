.DEFAULT_GOAL := help
# Shared companions: vendor/toko-tooling/make/cargo-dependencies.mk
.PHONY: help test version ensure-clean fetch-dependencies check-pins check-production-graph \
        install-tools tools-check install-host-tools host-tools-check install-ic-tools ic-tools-check test-tools test-failure-evidence test-offline-tool-reuse \
        lint-tooling test-hooks test-tooling test-fixture-cell validate validate-toolchain wasm-size \
        install-runtime-server runtime-server-check test-runtime test-runtime-logging \
        install-release-tools release-tools-check \
        test-release-adapters test-package-admission validation-compilers-check \
        release-version release-preflight release-verify release-prepare-version \
        release-prepared-check release-files release-commit-check release-committed-check \
        release-tagged-check release-push-check qualify-release package publish publish-dry-run

export RELEASE_GROUP
ifneq ($(filter release-resume qualify-release publish publish-dry-run,$(MAKECMDGOALS)),)
ifeq ($(strip $(RELEASE_GROUP)),)
$(error Select RELEASE_GROUP explicitly for release, resume or publication)
endif
endif
ifneq ($(filter release-patch release-minor release-major,$(MAKECMDGOALS)),)
ifeq ($(strip $(RELEASE_GROUP)$(RELEASE_GROUPS)),)
$(error Select RELEASE_GROUP or an explicit supplier-first RELEASE_GROUPS roster)
endif
endif
export RELEASE_DELIVERY ?= direct
ifneq ($(filter release-%,$(MAKECMDGOALS)),)
ifneq ($(RELEASE_DELIVERY),direct)
$(error ic-storage release adapters support only RELEASE_DELIVERY=direct)
endif
endif

# Bootstrap from the repository's simple, checked-in toolchain declaration.
# The Rust helper parses the full TOML for its own compiler identity checks.
VALIDATION_TOOLCHAIN ?= $(shell sed -n 's/^channel = "\([^"]*\)"$$/\1/p' rust-toolchain.toml)
include ci/release-tools.env
include ci/runtime-tools.env
TOOL := bash scripts/dev/run-repo-tool.sh $(VALIDATION_TOOLCHAIN)
override TOKO_TOOLING_ROOT := $(CURDIR)/vendor/toko-tooling
# Consumer roster; Shared owns each visible pass, including standalone Rustfmt.
FORMAT_RUST_ARGS = --extra-workspace qualification \
    --rustfmt-source testing/package-consumer.rs --rustfmt-edition 2024
# Memory uses IC executables for installed runtime qualification.
IC_TOOLS := wasm-opt
LOCAL_TOOL_INSTALL_TARGETS += install-ic-tools install-release-tools
LOCAL_TOOL_CHECK_TARGETS += ic-tools-check release-tools-check
RELEASE_SETUP_TARGET := release-candidate-setup
TOKO_TOOLING_RELEASE := 1
include vendor/toko-tooling/make/consumer.mk
include vendor/toko-tooling/make/cargo-dependencies.mk
export PATH := $(CURDIR)/.tools/ic/bin:$(PATH)
# GNU Make 3.81 cannot combine target-specific override and export. Reclassify
# caller values before applying the formatter-only exported compiler policy.
ifneq ($(origin RUSTUP_TOOLCHAIN),undefined)
override RUSTUP_TOOLCHAIN := $(RUSTUP_TOOLCHAIN)
endif
fmt fmt-check format-tools-check: export RUSTUP_TOOLCHAIN := $(VALIDATION_TOOLCHAIN)

# The published Testkit CLI owns PocketIC selection, admission and lifecycle.
# Keep this tool graph outside both maintained workspace lockfiles.
TESTKIT_CLI := RUSTUP_TOOLCHAIN=$(VALIDATION_TOOLCHAIN) bash vendor/toko-tooling/scripts/dev/install-rust-tools.sh --consumer "$(CURDIR)" --package ic-testkit --version $(IC_TESTKIT_VERSION) --bin ic-testkit-server --profile release --features cli
TESTKIT_SERVER_DIRECTORY := $(CURDIR)/.tools/ic-testkit-server

help:
	@echo 'Setup: install-tools (host, IC, Cargo tools), fetch-dependencies, install-hooks; prepare the Rust toolchain first.'
	@echo 'Dependencies: deps-list (offline package/version/group inventory); deps-update (Cargo updates root and qualification sequentially; optional DEPENDENCY_PACKAGE and DEPENDENCY_PRECISE selectors).'
	@echo 'Focused checks: toko-tooling-check, tools-check, test-tools, test-failure-evidence, check-pins, check-production-graph, check-storage-boundary, check-candid-boundary, test-tooling, test-fixture-cell, test-release-adapters, test-hooks, fmt-check, lint-tooling.'
	@echo 'Formatting: fmt. Full release checks: validate, validate-toolchain.'
	@echo 'Installed runtime: install-runtime-server (explicit Testkit setup), runtime-server-check (offline), test-runtime (prepared server and locked caches).'
	@echo 'Focused installed Logging proof: test-runtime-logging (original Wasm only; no optimizer or other runtime suites).'
	@echo 'Map comparison: benchmark-storage (prepared server/caches; BENCH_ROWS=256 BENCH_SAMPLES=3).'
	@echo 'Consumer Wasm size: benchmark-storage-size (matched small apps; prepared server/caches).'
	@echo 'LOC: toko-cloc (repository); --manifest Cargo.toml (product) or --manifest qualification/Cargo.toml (qualification).'
	@echo 'Maintainer releases: release-patch, release-minor, release-major; normal targets start fresh; release-resume VERSION=X.Y.Z recovers a saved attempt.'

# Explicit preparation; normal checks only admit the selected published editor.
install-release-tools:
	+RUSTUP_TOOLCHAIN=$(VALIDATION_TOOLCHAIN) bash vendor/toko-tooling/scripts/dev/install-rust-tools.sh --consumer "$(CURDIR)" --package ic-build --version $(IC_BUILD_RELEASE_METADATA_VERSION) --bin ic-build-release-metadata --profile debug --features cli

release-tools-check:
	+RUSTUP_TOOLCHAIN=$(VALIDATION_TOOLCHAIN) bash vendor/toko-tooling/scripts/dev/install-rust-tools.sh --consumer "$(CURDIR)" --package ic-build --version $(IC_BUILD_RELEASE_METADATA_VERSION) --bin ic-build-release-metadata --profile debug --features cli --check

# Explicit network setup; ordinary tool/library gates never provision a server.
install-runtime-server: host-tools-check
	+@cli="$$($(TESTKIT_CLI))" && "$$cli" setup --directory "$(TESTKIT_SERVER_DIRECTORY)"

# Both CLI receipt/bytes and server archive/bytes are checked without downloads.
runtime-server-check:
	+@{ cli="$$($(TESTKIT_CLI) --check)" && "$$cli" check --directory "$(TESTKIT_SERVER_DIRECTORY)"; } || \
	  { status=$$?; if [ "$$status" -ne 75 ]; then \
	      echo 'Prepare the selected CLI and server with make install-runtime-server.' >&2; \
	    fi; exit "$$status"; }

# Focused installed IO/upgrade qualification, separate from the library gates.
# Build before launch so compilation does not consume the server lifetime.
test-runtime: runtime-server-check ic-tools-check
	+cargo +$(VALIDATION_TOOLCHAIN) build --manifest-path qualification/Cargo.toml --target-dir target -p ic-storage-test-fixtures --locked --offline --profile wasm-size --target wasm32-unknown-unknown --example wasm-io-qualification --example wasm-storage-qualification --example wasm-logging-qualification
	+cargo +$(VALIDATION_TOOLCHAIN) build --locked --offline --manifest-path qualification/Cargo.toml --target-dir target/runtime-qualification
	+@cli="$$($(TESTKIT_CLI) --check)" && \
	  bash scripts/ci/qualify-runtime.sh "$(CURDIR)" "$$cli" "$(TESTKIT_SERVER_DIRECTORY)" "$(CURDIR)/.tools/ic/bin/wasm-opt" "$(IC_TOOL_PINS)"

test-runtime-logging: runtime-server-check
	+cargo +$(VALIDATION_TOOLCHAIN) build --manifest-path qualification/Cargo.toml --target-dir target -p ic-storage-test-fixtures --locked --offline --profile wasm-size --target wasm32-unknown-unknown --example wasm-logging-qualification
	+cargo +$(VALIDATION_TOOLCHAIN) build --locked --offline --manifest-path qualification/Cargo.toml --target-dir target/runtime-qualification
	+@cli="$$($(TESTKIT_CLI) --check)" && \
	  bash scripts/ci/qualify-runtime.sh "$(CURDIR)" "$$cli" "$(TESTKIT_SERVER_DIRECTORY)" - "$(IC_TOOL_PINS)" --logging-only

# Explicit comparison only; excluded from product release and validation gates.
BENCH_ROWS ?= 256
# The ordered/iterator sweeps already cover many batch sizes and three rounds.
BENCH_SAMPLES ?= $(if $(filter iterators ordered auth-lookups types,$(BENCH_WORKLOAD)),1,3)
.PHONY: benchmark-storage
benchmark-storage: runtime-server-check
	+@cli="$$($(TESTKIT_CLI) --check)" && \
	  RUSTUP_TOOLCHAIN=$(VALIDATION_TOOLCHAIN) bash scripts/ci/compare-storage.sh "$(CURDIR)" "$$cli" "$(TESTKIT_SERVER_DIRECTORY)" "$(BENCH_ROWS)" "$(BENCH_SAMPLES)"

# Small consumer footprints; separate from the broad performance catalogue.
.PHONY: benchmark-storage-size
benchmark-storage-size: runtime-server-check
	+@cli="$$($(TESTKIT_CLI) --check)" && \
	  RUSTUP_TOOLCHAIN=$(VALIDATION_TOOLCHAIN) bash scripts/ci/compare-storage-size.sh "$(CURDIR)" "$$cli" "$(TESTKIT_SERVER_DIRECTORY)"

test-failure-evidence:
	bash scripts/ci/test-failure-evidence.sh $${FAILURE_EVIDENCE_DIRECTORY:+"$$FAILURE_EVIDENCE_DIRECTORY"}

# Memory owns its archive bindings and fixture retention. Generic installer,
# runner, Make/LOC and parser matrices qualify at Shared Tooling's owner.
test-tools: test-failure-evidence test-ci-selection
	bash scripts/ci/test-consumer-fixture-retention.sh

# Actual workflow selection and real documentation checks, with Git substitutes.
.PHONY: test-ci-selection
test-ci-selection:
	bash scripts/ci/test-ci-selection.sh

# Uses only explicitly prepared tool generations; never prepares missing selections.
test-offline-tool-reuse:
	+bash scripts/ci/test-offline-tool-reuse.sh

# Network preparation is separate from offline checks; preserve tracked locks.
fetch-dependencies:
	+cargo +$(VALIDATION_TOOLCHAIN) fetch --locked

.PHONY: check-runtime-tooling-selection
check-pins: check-runtime-tooling-selection
check-runtime-tooling-selection:
	@root="$$(perl vendor/toko-tooling/scripts/dev/tooling-lock.pl "$(CURDIR)")" && \
	  runtime="$$(perl vendor/toko-tooling/scripts/dev/tooling-lock.pl "$(CURDIR)/qualification")" && \
	  test "$$root" = "$$runtime" || { echo 'Workspace tooling selections must agree.' >&2; exit 1; }

check-pins:
	+RUSTUP_TOOLCHAIN=$(VALIDATION_TOOLCHAIN) RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true bash vendor/toko-tooling/scripts/ci/check-dependency-pins.sh --cargo-inheritance

# Product policy stays here; Shared observes the locked normal/build graph.
.PHONY: check-dependency-adoption
check-dependency-adoption:
	+bash scripts/ci/check-dependency-adoption.sh $(VALIDATION_TOOLCHAIN)

check-production-graph:
	+RUSTUP_TOOLCHAIN=$(VALIDATION_TOOLCHAIN) RUSTUP_AUTO_INSTALL=0 perl vendor/toko-tooling/scripts/ci/check-cargo-graph.pl \
		--consumer "$(CURDIR)" --package ic-storage --package ic-metrics-storage --package ic-logging-storage --target wasm32-unknown-unknown \
		--forbid ic-build --forbid-pattern '^ic-host-' --forbid-pattern '^ic-testkit(?:-|$$)' \
		--forbid-pattern '^clap(?:_|$$)' \
		--forbid pocket-ic --forbid serde_json --forbid toml --forbid trybuild \
		--forbid-feature ic-storage/test-support \
		--forbid-feature ic-platform-runtime/runtime --forbid-feature ic-platform-runtime/executor --forbid-feature ic-platform-runtime/cdk \
		--forbid ic-platform-runtime-guard --forbid ic-platform-types --forbid candid --forbid-pattern '^ic-cdk' \
		--forbid ic-stable-structures --require ic-storage=1 --forbid ic-platform --require ic-platform-runtime=1 --require ic-metrics=1 --require ic-logging=1 --require ic-wire=1

	+RUSTUP_TOOLCHAIN=$(VALIDATION_TOOLCHAIN) RUSTUP_AUTO_INSTALL=0 perl vendor/toko-tooling/scripts/ci/check-cargo-graph.pl \
		--consumer "$(CURDIR)" --package ic-storage --package ic-metrics-storage --package ic-logging-storage \
		--target "$$(rustc +$(VALIDATION_TOOLCHAIN) -vV | sed -n 's/^host: //p')" \
		--require ic-wire=1 --forbid-feature ic-metrics/native-wire \
		--forbid ic-platform --forbid ic-platform-runtime --forbid ic-platform-types --forbid ic-platform-runtime-guard

# Install exact, checksum-verified tools separately; these checks are offline.
lint-tooling: check-shellcheck-stage
	$${ACTIONLINT_BIN:-actionlint} .github/workflows/*.yml

.PHONY: check-shellcheck-stage test-validation-stage-reuse
check-shellcheck-stage:
	+bash scripts/ci/check-shellcheck-stage.sh $(VALIDATION_TOOLCHAIN)

test-validation-stage-reuse:
	bash scripts/ci/test-validation-stage-reuse.sh

test-tooling:
	+cargo +$(VALIDATION_TOOLCHAIN) test --locked --offline --example repo-tool

test-release-adapters:
	bash scripts/ci/test-lint-policy.sh $(VALIDATION_TOOLCHAIN)
	bash scripts/ci/test-validation-cargo.sh
	bash scripts/ci/test-release-group-plans.sh
	bash scripts/ci/test-release-adapters.sh
	bash scripts/ci/test-consolidated-bootstrap.sh
	bash scripts/ci/test-dependency-adoption.sh

validation-compilers-check:
	bash scripts/ci/check-validation-compilers.sh $(VALIDATION_TOOLCHAIN)

test-package-admission:
	bash scripts/ci/test-validation-compilers.sh
	bash scripts/ci/test-package-admission.sh

test-hooks: format-tools-check
	+CDPATH="$(CURDIR)" RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true bash scripts/ci/test-git-hooks.sh

test:
	+cargo +$(VALIDATION_TOOLCHAIN) test --workspace --locked --offline -- --test-threads=1

test-fixture-cell:
	+cargo +$(VALIDATION_TOOLCHAIN) clippy -p ic-storage --locked --offline --lib --tests --features test-support -- -D warnings
	+cargo +$(VALIDATION_TOOLCHAIN) test -p ic-storage --locked --offline --lib --features test-support adapter::tests::cells:: -- --test-threads=1
	+cargo +$(VALIDATION_TOOLCHAIN) check -p ic-storage --locked --offline --target wasm32-unknown-unknown --lib --features test-support

version:
	+@$(TOOL) version

ensure-clean:
	+@$(TOOL) ensure-clean

# The full CI roster is also the exhaustive release-selection policy.
# ci/release-validation.json is the single full-stage roster.
.PHONY: validate-admission validate-lint validate-features validate-policy validate-tooling validate-storage-tests validate-metrics-tests validate-logging-tests validate-docs validate-wasm validate-wasm-size validate-archives validate-release-plan
validate:
	$(MAKE) --no-print-directory validate-toolchain

.PHONY: validate-ci
validate-ci:
	+$(TOOL) validate-ci --event "$${CHANGE_EVENT:-}" --base "$${CHANGE_BASE:-}" --head "$${CHANGE_HEAD:-}" --reference "$${CHANGE_REF:-}"

validate-toolchain:
	+bash scripts/ci/validate-storage-workspace.sh

# Only the admitted native release adapter supplies this plan and source identity.
validate-release-plan:
	+VALIDATION_REPOSITORY_ROOT="$(CURDIR)" RELEASE_SOURCE="$(RELEASE_SOURCE)" RELEASE_GROUP="$(RELEASE_GROUP)" VALIDATION_LOG_DIR="$(VALIDATION_LOG_DIR)" bash vendor/toko-tooling/scripts/ci/run-validation-targets.sh --plan "$(VALIDATION_PLAN)"

validate-admission:
	$(MAKE) --no-print-directory toko-tooling-check tools-check format-tools-check validation-compilers-check

validate-lint:
	+bash scripts/ci/validation-cargo.sh lint $(VALIDATION_TOOLCHAIN) "$(VALIDATION_PLAN)"

validate-features:
	+bash scripts/ci/validation-cargo.sh features $(VALIDATION_TOOLCHAIN) "$(VALIDATION_PLAN)"

validate-policy:
	$(MAKE) --no-print-directory check-production-graph check-pins check-storage-boundary check-candid-boundary check-license check-doc-links fmt-check

validate-tooling:
	$(MAKE) --no-print-directory test-release-adapters test-tools test-tooling test-hooks test-package-admission

validate-storage-tests:
	$(MAKE) --no-print-directory test-fixture-cell
	+cargo +$(VALIDATION_TOOLCHAIN) test -p ic-storage --locked --offline -- --test-threads=1 --skip repo_tool::

validate-metrics-tests:
	+cargo +$(VALIDATION_TOOLCHAIN) test -p ic-metrics-storage --locked --offline -- --test-threads=1

validate-logging-tests:
	+cargo +$(VALIDATION_TOOLCHAIN) test -p ic-logging-storage --locked --offline -- --test-threads=1

validate-docs:
	+bash scripts/ci/validation-cargo.sh docs $(VALIDATION_TOOLCHAIN) "$(VALIDATION_PLAN)"

.PHONY: check-qualification-lints check-qualification-docs
check-qualification-lints:
	+bash scripts/ci/check-qualification-lints.sh $(VALIDATION_TOOLCHAIN)

check-qualification-docs:
	+bash scripts/ci/check-qualification-docs.sh $(VALIDATION_TOOLCHAIN)

validate-wasm:
	+bash scripts/ci/validation-cargo.sh wasm $(VALIDATION_TOOLCHAIN) "$(VALIDATION_PLAN)"

validate-wasm-size:
	$(MAKE) --no-print-directory wasm-size

validate-archives:
	+bash scripts/ci/package-workspace.sh $(VALIDATION_TOOLCHAIN) --allow-dirty

wasm-size:
	+@$(TOOL) wasm-size

# Preserve release-only cache preparation for all caller settings.
# GNU Make 3.81 cannot combine target-specific override and export. Reclassify
# caller values first so these exported target policies can supersede them.
ifneq ($(origin RELEASE_CACHE_PREPARE),undefined)
override RELEASE_CACHE_PREPARE := $(RELEASE_CACHE_PREPARE)
endif
release-patch release-minor release-major release-resume: export RELEASE_CACHE_PREPARE := 1

.PHONY: release-candidate-setup
# Consumer deployment paths outside Shared's standard release targets.
clean-empty-source-dirs release-candidate-setup release-batch-resume release-prepare-version qualify-release publish publish-dry-run: toko-tooling-check

release-candidate-setup:
	+@case "$(RELEASE_SETUP_MODE)" in \
	  prepare) $(MAKE) --no-print-directory install-tools fetch-dependencies ;; \
	  check) $(MAKE) --no-print-directory tools-check format-tools-check ;; \
	  *) echo 'Select RELEASE_SETUP_MODE=prepare or check' >&2; exit 2 ;; \
	esac

release-version:
	+@$(TOOL) version

release-preflight release-prepare-version release-prepared-check release-files release-commit-check release-committed-check release-tagged-check release-push-check:
	+@$(TOOL) $@

# Preserve every gate attempt, including failures. The adapter owns the gate;
# these logs use Cargo's selected target directory, including overrides.
release-verify:
	+@set -eu; \
	log_dir="$$($(TOOL) target)/release-validation/attempts"; \
	mkdir -p "$$log_dir"; \
	log_file="$$(mktemp "$$log_dir/verify.XXXXXX")"; \
	if $(TOOL) release-verify > "$$log_file" 2>&1; then result=0; else result=$$?; fi; \
	cat "$$log_file"; \
	echo "Retained release-gate log: $$log_file"; \
	exit "$$result"

qualify-release:
	+$(TOOL) qualify-release

package: ensure-clean
	+bash scripts/ci/package-workspace.sh $(VALIDATION_TOOLCHAIN)

publish:
	+$(TOOL) publish

publish-dry-run:
	+$(TOOL) publish --dry-run

.PHONY: check-storage-boundary check-candid-boundary check-license check-doc-links
check-storage-boundary:
	+RUSTUP_TOOLCHAIN=$(VALIDATION_TOOLCHAIN) RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true bash vendor/toko-tooling/scripts/ci/check-cargo-dependency-boundaries.sh --consumer "$(CURDIR)" --external-ic
	+RUSTUP_TOOLCHAIN=$(VALIDATION_TOOLCHAIN) RUSTUP_AUTO_INSTALL=0 CARGO_NET_OFFLINE=true bash vendor/toko-tooling/scripts/ci/check-cargo-dependency-boundaries.sh --consumer "$(CURDIR)/qualification"

# One lexical inventory covers packages, tools, benchmarks and runtime fixtures.
check-candid-boundary:
	perl vendor/toko-tooling/scripts/ci/check-candid-boundary.pl --cbor "$(CURDIR)" ci/candid-boundary.json crates qualification testing

check-license:
	perl vendor/toko-tooling/scripts/ci/check-license.pl --file LICENSE.md

check-doc-links:
	perl vendor/toko-tooling/scripts/ci/check-documentation-links.pl --root "$(CURDIR)" README.md AGENTS.md docs/status/current.md docs/validation/storage-release-validation.md docs/migration/ic-persistence.md docs/migration/ic-metrics-stable.md docs/storage/upstreams.md crates/ic-storage/README.md crates/ic-metrics-storage/README.md crates/ic-logging-storage/README.md docs/archive/private-btree/*.md docs/history/ic-storage/index.md
