//! The block actions on a real shell: copy the command, save the output to Downloads.

use super::super::support::tools_present;
use super::support::{pump, run_line, shell_path, start};
use temor_core::block::Outcome;
use temor_vt::SurfacePoint;

#[test]
fn copy_command_puts_the_typed_command_on_the_clipboard() {
    if !tools_present() {
        return;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start("blocks-copy", &shell);
    run_line(&live, "echo copy-me");
    pump(&mut live, "the block", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    let id = live.session.block_outcomes()[0].0;
    live.session.copy_command(id);
    assert_eq!(live.clipboard.0.borrow().as_str(), "echo copy-me");
}

#[test]
fn save_output_places_the_output_in_downloads_under_its_command() {
    if !tools_present() {
        return;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start("blocks-save", &shell);
    run_line(&live, "echo saved-out");
    pump(&mut live, "the block", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    let id = live.session.block_outcomes()[0].0;
    live.session.save_output(id).expect("save");
    assert_eq!(
        live.session.take_notice().as_deref(),
        Some("Saved to Downloads.")
    );
    let saved = live.shelf.join("echo saved-out output.txt");
    assert_eq!(
        std::fs::read_to_string(&saved).expect("saved file"),
        "saved-out"
    );
    // Nothing else is left in the shelf folder: the temporary file was moved, not copied.
    let entries: Vec<_> = std::fs::read_dir(&live.shelf)
        .expect("shelf")
        .map(|e| e.expect("entry").file_name())
        .collect();
    assert_eq!(entries.len(), 1, "{entries:?}");
}

#[test]
fn edit_locally_names_the_blocks_file_wherever_the_pointer_last_was() {
    if !tools_present() {
        return;
    }
    let shell = shell_path("bash").expect("bash");
    let mut live = start("blocks-edit-text", &shell);
    run_line(&live, "echo notes.md");
    pump(&mut live, "the block", |s| {
        s.block_outcomes()
            .first()
            .is_some_and(|b| b.1 != Outcome::Running)
    });
    let id = live.session.block_outcomes()[0].0;
    // The pointer last crossed a grid cell, then went onto the toolbar: the word under that cell
    // is not what the toolbar edits.
    live.session.pointer_moved(SurfacePoint { x: 2, y: 2 });
    assert_eq!(
        live.session.block_text_to_edit(Some(id)).as_deref(),
        Some("notes.md")
    );
}
