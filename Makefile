NAME       := srv
HOST       := $(shell rustc -vV | awk '/^host:/ {print $$2}')
TARGET     ?= $(subst -unknown-linux-gnu,-unknown-linux-musl,$(HOST))
TARGET_ENV := $(shell printf '%s' "$(TARGET)" | tr '[:lower:]-' '[:upper:]_')
CARGO_CMD  := $(if $(findstring -linux-musl,$(TARGET)),$(shell command -v musl-cargo 2>/dev/null || printf cargo),cargo)
comma      := ,
# Setting RUSTFLAGS replaces the target rustflags from .cargo/config.toml, so the
# macOS unused-dylib link argument is repeated for release builds.
RELEASE_RUSTFLAGS := -Zlocation-detail=none -Zunstable-options -Cpanic=immediate-abort $(if $(findstring apple-darwin,$(TARGET)),-Clink-arg=-Wl$(comma)-dead_strip_dylibs)
cargo = $(if $(findstring -linux-musl,$(TARGET)),,$(if $(filter release-static test,$(1)),CARGO_TARGET_$(TARGET_ENV)_LINKER=clang ,)$(if $(filter release-static,$(1)),RUSTFLAGS="$(RELEASE_RUSTFLAGS)")) MUSL_TARGET="$(TARGET)" MUSL_BUILD_MODE="$(1)" $(CARGO_CMD)

.PHONY: build test test-ci lint coverage mutants-diff release verify-release install

build:
	$(call cargo,dev) build

test:
	$(call cargo,test) test --quiet

test-ci:
	$(call cargo,test) test --quiet --release

lint:
	cargo clippy --all-targets --all-features -- --deny warnings

# Coverage is a report for finding unreached code, never a gate.
coverage:
	cargo llvm-cov --branch --html --output-dir .work/coverage
	cargo llvm-cov report --branch --summary-only

# Mutation-test the Rust lines changed since BASE. A diff that changes only
# tests selects no mutants, so it tests every mutant in the crate instead.
BASE    ?= origin/main
MUTANTS := cargo mutants --baseline run --timeout 60 --build-timeout 300 --output .work/mutants
mutants-diff:
	mkdir -p .work/mutants
	git diff --merge-base "$(BASE)" -- '*.rs' > .work/mutants/pr.diff
	if [ ! -s .work/mutants/pr.diff ]; then \
	  echo "no Rust changes against $(BASE)"; \
	elif [ -n "$$(cargo mutants --list --in-diff .work/mutants/pr.diff)" ]; then \
	  $(MUTANTS) --in-diff .work/mutants/pr.diff; \
	else \
	  echo "the diff changes only tests; testing every mutant in the crate"; \
	  $(MUTANTS); \
	fi

release:
	$(call cargo,release-static) clean -p $(NAME) --profile dist --target $(TARGET)
	$(call cargo,release-static) build --profile dist \
	  -Z build-std=std \
	  -Z build-std-features= \
	  --target $(TARGET)

verify-release:
	@test -f "target/$(TARGET)/dist/$(NAME)"
	@if echo "$(TARGET)" | grep -q -- '-linux-musl$$'; then \
		command -v readelf >/dev/null || { echo 'readelf is required for release verification'; exit 1; }; \
		file "target/$(TARGET)/dist/$(NAME)" | grep -Eq 'static-pie linked|statically linked' || { echo 'release is not statically linked'; exit 1; }; \
		file "target/$(TARGET)/dist/$(NAME)" | grep -q 'stripped' || { echo 'release is not stripped'; exit 1; }; \
		! readelf -l "target/$(TARGET)/dist/$(NAME)" | grep -q INTERP || { echo 'release has a dynamic ELF interpreter'; exit 1; }; \
		! readelf -d "target/$(TARGET)/dist/$(NAME)" | grep -q NEEDED || { echo 'release has dynamic dependencies'; exit 1; }; \
	else \
		echo "Skipping ELF checks for $(TARGET)"; \
	fi

install: release
	cp target/$(TARGET)/dist/$(NAME) ~/.local/bin/$(NAME)
	@if test "$$(uname -s)" = Darwin; then \
		codesign -fs - ~/.local/bin/$(NAME); \
	fi
