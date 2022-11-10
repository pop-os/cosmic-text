#!/usr/bin/env bash

set -ex

echo Build with default features
cargo build --release

echo Build with no default features
cargo build --release --no-default-features

echo Build with only std feature
cargo build --release --no-default-features --features std

echo Build with only swash feature
cargo build --release --no-default-features --features swash

echo Build with only syntect feature
cargo build --release --no-default-features --features syntect

echo Build with all features
cargo build --release --all-features

echo Run tests
cargo test
