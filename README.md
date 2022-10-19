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
