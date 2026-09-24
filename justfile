# Copyright 2026 RegaliumOS™.
# SPDX-License-Identifier: Apache-2.0

# The commands CI runs, so that "it passes locally" and "it passes in CI" are
# the same claim. Anything CI does that is not a recipe here is a way for the
# two to drift apart.

default:
    @just --list

# --- conformance -----------------------------------------------------------

# Architecture, size, purity, dependency and licence gates. Run this before you
# think you are done.
[group('conformance')]
[doc('Architecture, size, purity, dependency and licence gates')]
gates *gate:
    cargo run -q -p xtask -- {{gate}}

# The gates, plus the negative tests that prove each one fails when it should.
# A gate exercised only against a correct tree reports success both when it
# works and when it does nothing.
[group('conformance')]
[doc('The gates, plus the negative tests proving each one bites')]
conformance: gates
    cargo test -q -p xtask

# --- test ------------------------------------------------------------------

[group('test')]
test:
    cargo test --workspace --all-targets

# The phase gate: every committed corpus decodes, re-encodes byte for byte, and
# means what the record says it means.
#
# The first corpus lands in Phase 3 with the tag codec. Until then this recipe
# says so rather than reporting a pass it has not earned -- and once a corpus
# exists it becomes a hard gate, because a corpus that silently asserts nothing
# is worse than no corpus.
[group('test')]
[doc('The phase gate: every committed corpus round-trips byte for byte')]
fixtures:
    #!/usr/bin/env bash
    set -euo pipefail
    corpora=$(find crates -name '*.hex' -o -name '*.kat' 2>/dev/null | wc -l | tr -d ' ')
    if [ "$corpora" -eq 0 ]; then
        echo "no fixture corpus yet; the first lands in Phase 3 with the tag codec"
        exit 0
    fi
    echo "walking $corpora corpora"
    cargo test --workspace -- fixture

[group('test')]
fuzz duration='60s':
    #!/usr/bin/env bash
    set -euo pipefail
    echo "fuzz targets land in Phase 17; {{duration}} each once they exist"

# --- lint ------------------------------------------------------------------

[group('lint')]
lint:
    cargo fmt --all -- --check
    cargo clippy --workspace --all-targets -- -D warnings

[group('lint')]
fmt:
    cargo fmt --all

# Licences and advisories across the whole resolved graph.
[group('lint')]
deny:
    cargo deny check

# Google AIP lint over the schema tree. Skips rather than fails while the tree
# is empty: the linter treats no input as an error, and a red first commit
# teaches people to ignore the gate.
[group('lint')]
[doc('Google AIP lint over the schema tree')]
api-lint:
    #!/usr/bin/env bash
    set -euo pipefail
    if [ -z "$(find protobuf -name '*.proto' 2>/dev/null)" ]; then
        echo "no .proto yet; the schema tree is populated in Phase 9"
        exit 0
    fi
    cd protobuf && api-linter --set-exit-status $(find . -name '*.proto')

# --- build -----------------------------------------------------------------

[group('build')]
build:
    cargo build --workspace

# Every target CI builds. Pure Rust is what makes this one flag per target
# rather than a cross-compilation project, and `just gates` is what keeps it
# that way by refusing a dependency that needs a C toolchain.
[group('build')]
[doc('Build for every target CI builds')]
cross:
    #!/usr/bin/env bash
    set -euo pipefail
    for target in armv7-unknown-linux-gnueabihf aarch64-unknown-linux-gnu \
                  x86_64-unknown-linux-gnu; do
        echo "::group::$target"
        cargo check --workspace --target "$target"
        echo "::endgroup::"
    done

# Everything CI runs, in the order it runs it.
[group('build')]
ci: lint conformance test fixtures
