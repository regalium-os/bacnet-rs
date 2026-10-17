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
    # The walkers compile their corpora in, so a file they do not name is a
    # file nothing reads.
    for f in $(find crates -name '*.hex' -o -name '*.kat'); do
        if ! grep -rqs "$(basename "$f")" "$(dirname "$(dirname "$(dirname "$f")")")/tests"; then
            echo "$f is not named by any fixture walker, so nothing decodes it"
            exit 1
        fi
    done
    echo "walking $corpora corpora"
    out=$(cargo test --workspace -- fixture 2>&1) || { echo "$out"; exit 1; }
    # A filter that matches no test passes having checked nothing.
    ran=$(echo "$out" | awk '/^test result:/ {n += $4} END {print n + 0}')
    if [ "$ran" -eq 0 ]; then
        echo "no fixture test ran over $corpora corpora; the walker was renamed or removed"
        exit 1
    fi
    echo "$ran fixture test(s) passed"

[group('test')]
[doc('Fuzz every decoder for DURATION seconds each, on nightly')]
fuzz duration='60':
    #!/usr/bin/env bash
    set -euo pipefail
    # libFuzzer needs nightly; put its rustc ahead of any other on PATH.
    export PATH="$(dirname "$(rustup which --toolchain nightly rustc)"):$PATH"
    cd fuzz
    for target in $(cargo fuzz list); do
        echo "fuzz $target for {{duration}} s"
        cargo fuzz run "$target" -- -max_total_time={{duration}}
    done

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
    # Through a buf image, as CI does: the schema imports googleapis and
    # protovalidate, which buf resolves and the linter alone cannot.
    image="$(mktemp -t bacnet-image.XXXXXX)"
    trap 'rm -f "$image"' EXIT
    buf lint
    buf format --diff --exit-code
    buf build -o "$image"
    cd protobuf && api-linter --set-exit-status --descriptor-set-in "$image" \
        $(find . -name '*.proto')

# --- codegen ---------------------------------------------------------------

# Regenerate the schema crate's Rust types from protobuf/. The output is
# committed, so a consumer builds without buf or protoc.
[group('codegen')]
[doc('Regenerate the schema crate from protobuf/')]
gen:
    buf generate --template tools/protobuf/buf/rust.buf.gen.yaml

# Regenerate the FlatBuffers and Cap'n Proto mirrors of the hot path, and the
# ordinal ledger, from a descriptor set narrowed to what a COV notification
# needs. Needs buffers with the protovalidate and NullValue fixes (>= 0.2.2).
[group('codegen')]
[doc('Regenerate the hot-path mirrors and the ordinal ledger')]
gen-mirror:
    buf build --type bacnet.object.v1.StreamNotificationsResponse \
        -o protobuf/generated/payloads.descriptor.binpb
    buffers generate
    buffers verify

# Fails when the committed types are not what protobuf/ generates: a .proto
# edited without `just gen` ships a crate that disagrees with its schema.
[group('codegen')]
[doc('Fail if the generated Rust is stale')]
gen-check: gen
    git diff --exit-code -- crates/regalium-bacnet-schema/src/generated

# --- build -----------------------------------------------------------------

[group('build')]
build:
    cargo build --workspace

# Every target CI builds. Pure Rust is what makes this one flag per target
# rather than a cross-compilation project, and `just gates` is what keeps it
# that way by refusing a dependency that needs a C toolchain. The otel adapter
# and the probe carry the telemetry SDK's audited zstd/lz4, so they are
# checked by `cross-probe`, which needs a cross C compiler per target.
[group('build')]
[doc('Build the driver for every target CI builds')]
cross:
    #!/usr/bin/env bash
    set -euo pipefail
    for target in armv7-unknown-linux-gnueabihf aarch64-unknown-linux-gnu \
                  x86_64-unknown-linux-gnu; do
        echo "::group::$target"
        cargo check --workspace --target "$target" \
            --exclude regalium-bacnet-probe --exclude regalium-bacnet-telemetry-otel
        echo "::endgroup::"
    done

# The probe and the otel adapter for one target; needs that target's C compiler.
[group('build')]
[doc('Check the probe for TARGET; needs a cross C compiler')]
cross-probe target:
    cargo check --target {{target}} -p regalium-bacnet-probe -p regalium-bacnet-telemetry-otel

# The BACnet dashboard, imported into a running Grafana: devices, points,
# requests, traces and logs. Overwrites a dashboard with the same uid.
[group('build')]
[doc('Import the BACnet dashboard into Grafana at URL')]
grafana url='http://localhost:6009/grafana' auth='admin:admin':
    { printf '{"overwrite":true,"dashboard":'; cat tools/bacnet-probe/dist/grafana/bacnet.json; printf '}'; } \
        | curl -sf -u '{{auth}}' -H 'Content-Type: application/json' \
            -X POST '{{url}}/api/dashboards/db' --data @-
    @echo

# The probe, installed from this checkout.
[group('build')]
[doc('Install bacnet-probe from this checkout')]
probe:
    cargo install --locked --path tools/bacnet-probe

# Everything CI runs, in the order it runs it.
[group('build')]
ci: lint conformance test fixtures
