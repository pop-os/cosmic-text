# SPDX-License-Identifier: MIT OR Apache-2.0

RUST_LOG="cosmic_text=debug,editor_libcosmic_image=debug" cargo run --release --package editor-libcosmic-image -- "$@"
