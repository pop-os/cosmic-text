# COSMIC Text

Pure Rust multi-line text shaping and rendering for COSMIC.

COSMIC Text provides advanced text shaping, layout, and rendering wrapped up
into a simple abstraction. Shaping is provided by rustybuzz, and supports a
wide variety of advanced shaping operations. Rendering is provided by swash,
which supports ligatures and color emoji. Layout is implemented custom, in safe
Rust, and supports bidirectional text. Font fallback is also a custom
implementation, reusing some of the static fallback lists in browsers such as
Chromium and Firefox. Linux, macOS, and Windows are supported with the full
feature set. Other platforms may need to implement font fallback capabilities.

## Roadmap

The following features must be supported before this is "ready":

- [x] Font loading
  - [x] Preset fonts
  - [x] System fonts
- [ ] Text styles (bold, italic, etc.)
- [x] Font shaping (using rustybuzz)
  - [x] Cache results
  - [x] RTL
  - [x] Bidirectional rendering
- [x] Font fallback
  - [x] Choose font based on locale to work around "unification"
  - [x] Per-line granularity
  - [x] Per-character granularity
- [x] Font layout
  - [x] Click detection
  - [x] Simple wrapping
  - [ ] Wrapping with indentation
  - [ ] No wrapping
  - [ ] Ellipsize
- [x] Font rendering (using swash)
  - [x] Cache results
  - [x] Font hinting
  - [x] Ligatures
  - [x] Color emoji
- [x] Text editing
    - [x] Performance improvements
    - [x] Text selection
    - [x] Can automatically recreate https://unicode.org/udhr/ without errors (see below)
    - [ ] Bidirectional selection
    - [ ] Copy/paste

The UDHR (UN Declaration of Human Rights) test involves taking the entire set of
UDHR translations (almost 500 languages), concatenating them as one file (which
ends up being 8 megabytes!), then via the `editor-test` example, automatically
simulating the entry of that file into cosmic-text per-character, with the use
of backspace and delete tested per character and per line. Then, the final
contents of the buffer is compared to the original file. All of the 106746
lines are correct.

## License

Licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
