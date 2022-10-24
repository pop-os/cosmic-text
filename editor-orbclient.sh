# SPDX-License-Identifier: MIT OR Apache-2.0

RUST_LOG="cosmic_text=debug,editor_orbclient=debug" cargo run --release --package editor-orbclient -- "$@"
