# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.12.0] - 2024-06-18

### Added

- Cache codepoint support info for monospace fonts
- Store a sorted list of monospace font ids in font system
- Add line ending abstraction
- Horizontal scroll support in Buffer
- Concurrently load and parse fonts
- Add metrics to attributes
- Support expanding tabs
- Add an option to set selected text color
- Add Edit::cursor_position
- Allow layout to be calculated without specifying width
- Allow for undefined buffer width and/or height
- Add method to set syntax highlighting by file extension

### Fixed

- Fix no_std build
- Handle inverted Ranges in add_span
- Fix undo and redo updating editor modified status
- Ensure at least one line is in Buffer

### Changed

- Enable vi feature for docs.rs build
- Convert editor example to winit
- Refactor scrollbar width handling for editor example
- Convert rich-text example to winit
- Only try monospace fonts that support at least one requested script
- Skip trying monospace fallbacks if default font supports all codepoints
- Make vertical scroll by pixels instead of layout lines
- Upgrade dependencies and re-export ttf-parser

## [0.11.2] - 2024-02-08

### Fixed

- Fix glyph start and end when using `shape-run-cache`

## [0.11.1] - 2024-02-08

### Added

- Add `shape-run-cache` feature, that can significantly improve shaping performance

### Removed

- Remove editor-libcosmic, see cosmic-edit instead

## [0.11.0] - 2024-02-07

### Added

- Add function to set metrics and size simultaneously
- Cache `rustybuzz` shape plans
- Add capability to synthesize italic
- New wrapping option `WordOrGlyph` to allow word to glyph fallback

### Fixed

- `Buffer::set_rich_text`: Only add attrs if they do not match the defaults
- Do not use Emoji fonts as monospace fallback
- Refresh the attrs more often in basic shaping
- `Buffer`: fix max scroll going one line beyond end
- Improve reliability of `layout_cursor`
- Handle multiple BiDi paragraphs in `ShapeLine` gracefully
- Improved monospace font fallback
- Only commit a previous word range if we had an existing visual line

### Changed

- Update terminal example using `colored`
- Significant improvements for `Editor`, `SyntaxEditor`, and `ViEditor`
- Require default Attrs to be specified in `Buffer::set_rich_text`
- Bump `fontdb` to `0.16`
- Allow Clone of layout structs
- Move cursor motions to new `Motion` enum, move handling to `Buffer`
- Ensure that all shaping and layout uses scratch buffer
- `BufferLine`: user `layout_in_buffer` to implement layout
- `BufferLine`: remove wrap from struct, as wrap is passed to layout
- Refactor of scroll and shaping
- Move `color` and `x_opt` out of Cursor
- Add size limit to `font_matches_cache` and clear it when it is reached
- Update `swash` to `0.1.12`
- Set default buffer wrap to `WordOrGlyph`

## Removed
- Remove patch to load Redox system fonts, as fontdb does it now

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
