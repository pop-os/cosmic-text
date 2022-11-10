#!/usr/bin/env bash

function build {
    cargo build --release "$@"
    cargo clippy --no-deps "$@"
}

set -ex

echo Build with default features
build

echo Build with no default features
build --no-default-features

echo Build with only std feature
build --no-default-features --features std

echo Build with only swash feature
build --no-default-features --features swash

echo Build with only syntect feature
build --no-default-features --features syntect

echo Build with all features
build --all-features

echo Run tests
cargo test
