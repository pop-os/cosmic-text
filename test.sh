#!/usr/bin/env bash

set -ex

cargo doc
cargo test
cargo build --release --all
env RUST_LOG=editor_test=info target/release/editor-test
