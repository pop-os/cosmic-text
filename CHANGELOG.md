# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.10.0] - 2023-10-19

### Added

- Added `Buffer::set_rich_text` method
- Add `Align::End` for end-based alignment
- Add more `Debug` implementations
- Add feature to warn on missing glyphs
- Add easy conversions for tuples/arrays for `Color`
- Derive `Clone` for `AttrsList`
- Add feature to allow `fontdb` to get `fontconfig` information
- Add benchmarks to accurately gauge improvements
- Add image render tests
- Allow BSD-2-Clause and BSD-3-Clause licneses in cargo-deny

### Fixed

- Fix `no_std` build
- Fix `BufferLine::set_align` docs to not mention shape reset is performed
- Fix width computed during unconstrained layout and add test for it
- Set `cursor_moved` to true in `Editor::insert_string`
- Fix `NextWord` action in `Editor` when line ends with word boundaries
- Fix building `editor-libcosmic` with `vi` feature
- Respect `fontconfig` font aliases when enabled
- Fix rendering of RTL words

### Changed

- Unify `no_std` and `std` impls of `FontSystem`
- Move `hashbrown` behind `no_std` feature
- Require either `std` or `no_std` feature to be specified
- Use a scratch buffer to reduce allocations
- Enable `std` feature with `fontconfig` feature
- Enable `fontconfig` feature by default
- Refactor code in `ShapeLine::layout`
- Set MSRV to `1.65`
- Make `Edit::copy_selection` immutable
- Rewrite `PreviousWord` logic in `Editor` with iterators
- Use attributes at cursor position for insertions in `Editor`
- Update all dependencies
- Use `self_cell` for creating self-referential struct

## [0.9.0] - 2023-07-06

### Added

- Add `Shaping` enum to allow selecting the shaping strategy
- Add `Buffer::new_empty` to create `Buffer` without `FontSystem`
- Add `BidiParagraphs` iterator
- Allow setting `Cursor` color
- Allow setting `Editor` cursor
- Add `PhysicalGlyph` that allows computing `CacheKey` after layout
- Add light syntax highlighter to `libcosmic` example

### Fixed

- Fix WebAssembly support
- Fix alignment when not wrapping
- Fallback to monospaced font if Monospace family is not found
- Align glyphs in a `LayoutRun` to baseline

### Changed

- Update `fontdb` to 0.14.1
- Replace ouroboros with aliasable
- Use `BidiParagraphs` iterator instead of `str::Lines`
- Update `libcosmic` version

### Removed

- `LayoutGlyph` no longer has `x_int` and `y_int`, use `PhysicalGlyph` instead

## [0.8.0] - 2023-04-03

### Added

- `FontSystem::new_with_fonts` helper
- Alignment and justification
- `FontSystem::db_mut` provides mutable access to `fontdb` database
- `rustybuzz` is re-exported

### Fixed

- Fix some divide by zero panics
- Redox now uses `std` `FontSystem`
- Layout system improvements
- `BufferLinke::set_text` has been made more efficient
- Fix potential panic on window resize

### Changed

- Use `f32` instead of `i32` for lengths
- `FontSystem` no longer self-referencing
- `SwashCash` no longer keeps reference to `FontSystem`

### Removed

- `Attrs::monospaced` is removed, use `Family::Monospace` instead
