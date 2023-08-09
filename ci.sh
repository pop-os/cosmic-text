#!/usr/bin/env bash

function build {
    cargo build --release "$@"
    cargo clippy --no-deps "$@"
}

set -ex

echo Check formatting
cargo fmt --check

echo Build with default features
build

echo Build with only no_std feature
build --no-default-features --features no_std

echo Build with only std feature
build --no-default-features --features std

echo Build with only std and swash features
build --no-default-features --features std,swash

echo Build with only std and syntect features
build --no-default-features --features std,syntect

echo Build with only std and vi features
build --no-default-features --features std,vi

echo Build with all features
build --all-features

echo Run tests
cargo test
