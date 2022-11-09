#!/usr/bin/env bash

set -ex

cargo doc
cargo test
cargo build --release --no-default-features
cargo build --release --no-default-features --features std
cargo build --release --no-default-features --features swash
cargo build --release --no-default-features --features syntect
cargo build --release --all-features
cargo build --release --all
target/release/terminal
env RUST_LOG=editor_test=info target/release/editor-test
