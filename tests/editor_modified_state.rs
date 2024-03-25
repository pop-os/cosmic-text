use std::sync::OnceLock;

use cosmic_text::{Buffer, Change, Cursor, Edit, Metrics, SyntaxEditor, SyntaxSystem, ViEditor};

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

// Tests that inserting into an empty editor correctly sets the editor as modified.
#[test]
fn insert_in_empty_editor_sets_changed() {
    let mut editor = editor();

    assert!(!editor.changed());
    editor.start_change();
    editor.insert_at(Cursor::new(0, 0), "Robert'); DROP TABLE Students;--", None);
    editor.finish_change();
    assert!(editor.changed());
}

#[test]
fn insert_and_undo_in_unsaved_editor_is_unchanged() {
    let mut editor = editor();

    assert!(!editor.changed());
    editor.start_change();
    editor.insert_at(Cursor::new(0, 0), "loop {}", None);
    editor.finish_change();
    assert!(editor.changed());

    // Undoing the above change should set the editor as unchanged even if the save state is unset
    editor.start_change();
    editor.undo();
    editor.finish_change();
    assert!(!editor.changed());
}

#[test]
fn undo_to_last_save() {
    let mut editor = editor();

    // Latest saved change is the first change
    editor.start_change();
    let cursor = editor.insert_at(Cursor::new(0, 0), "Ferris is Rust's ", None);
    editor.finish_change();
    assert!(editor.changed());
    editor.save_point();
    // assert_eq!(Pivot::Saved, editor.save_pivot());
    assert!(!editor.changed());

    // A new insert should set the editor as modified and the pivot should still be on the first
    // change from earlier
    editor.start_change();
    editor.insert_at(cursor, "mascot", None);
    editor.finish_change();
    // assert_eq!(Pivot::Exact(0), editor.save_pivot());
    assert!(editor.changed());

    // Undoing the latest change should set the editor to unmodified again
    editor.start_change();
    editor.undo();
    editor.finish_change();
    // assert_eq!(Pivot::Saved, editor.save_pivot());
    assert!(!editor.changed());
}

#[test]
fn redoing_to_save_point_sets_editor_as_unchanged() {
    let mut editor = editor();

    // Initial change
    assert!(
        !editor.changed(),
        "Editor should start in an unchanged state"
    );
    editor.start_change();
    editor.insert_at(Cursor::new(0, 0), "editor.start_change();", None);
    editor.finish_change();
    assert!(
        editor.changed(),
        "Editor should be set as modified after insert() and finish_change()"
    );
    editor.save_point();
    assert!(
        !editor.changed(),
        "Editor should be unchanged after setting a save point"
    );

    // Change to undo then redo
    editor.start_change();
    editor.insert_at(Cursor::new(1, 0), "editor.finish_change()", None);
    editor.finish_change();
    assert!(
        editor.changed(),
        "Editor should be set as modified after insert() and finish_change()"
    );
    editor.save_point();
    assert!(
        !editor.changed(),
        "Editor should be unchanged after setting a save point"
    );

    editor.undo();
    assert!(
        editor.changed(),
        "Undoing past save point should set editor as changed"
    );
    editor.redo();
    assert!(
        !editor.changed(),
        "Redoing to save point should set editor as unchanged"
    );
}

#[test]
fn redoing_past_save_point_sets_editor_as_changed() {
    unimplemented!()
}

#[test]
fn undo_all_changes() {
    unimplemented!()
}
