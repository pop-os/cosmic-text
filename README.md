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
    - [ ] Can automatically recreate https://unicode.org/udhr/ without errors
    - [ ] Copy/paste
