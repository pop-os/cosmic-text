#!/usr/bin/env bash

set -ex

cargo build --release
cargo build --release --no-default-features
cargo build --release --no-default-features --features std
cargo build --release --no-default-features --features swash
cargo build --release --no-default-features --features syntect
cargo build --release --all-features
cargo build --release --all
