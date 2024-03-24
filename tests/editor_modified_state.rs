use std::sync::OnceLock;

use cosmic_text::{
    Buffer, Change, Cursor, Edit, Metrics, Pivot, SyntaxEditor, SyntaxSystem, ViEditor,
};

static SYNTAX_SYSTEM: OnceLock<SyntaxSystem> = OnceLock::new();

fn editor() -> ViEditor<'static, 'static> {
    // More or less copied from cosmic-edit
    let font_size: f32 = 14.0;
    let line_height = (font_size * 1.4).ceil();

    let metrics = Metrics::new(font_size, line_height);
    let buffer = Buffer::new_empty(metrics);
    let editor = SyntaxEditor::new(
        buffer,
        SYNTAX_SYSTEM.get_or_init(SyntaxSystem::new),
        "base16-eighties.dark",
    )
    .expect("Default theme `base16-eighties.dark` should be found");

    ViEditor::new(editor)
}

#[test]
fn undo_to_last_save() {
    let mut editor = editor();

    // Latest saved change is the first change
    editor.start_change();
    let cursor = editor.insert_at(Cursor::new(0, 0), "Ferris is Rust's ", None);
    editor.finish_change();
    assert!(editor.changed());
    editor.set_save_pivot(Pivot::Saved);
    assert_eq!(Pivot::Saved, editor.save_pivot());
    assert!(!editor.changed());

    // A new insert should set the editor as modified and the pivot should still be on the first
    // change from earlier
    editor.start_change();
    editor.insert_at(cursor, "mascot", None);
    editor.finish_change();
    assert_eq!(Pivot::Exact(0), editor.save_pivot());
    assert!(editor.changed());

    // Undoing the latest change should set the editor to unmodified again
    editor.start_change();
    editor.undo();
    editor.finish_change();
    assert_eq!(Pivot::Saved, editor.save_pivot());
    assert!(!editor.changed());
}
