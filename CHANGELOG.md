# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
