//! Unit tests for the mark helpers that need no terminal.

use temor_pane::text::{RowText, Wrap};

use super::actions::output_name;
use super::lead::leads_blank;
use temor_core::session::frame::Lead;

#[test]
fn output_names_come_from_the_command_or_a_fallback() {
    assert_eq!(output_name("make all"), "make all output.txt");
    assert_eq!(output_name("  "), "Command output.txt");
    assert_eq!(output_name(&"x".repeat(60)).len(), 40 + " output.txt".len());
}

#[test]
fn a_prompt_leads_with_a_blank_line_only_when_its_header_follows() {
    // (what, first row's text, input mark's row, lead); the prompt's first row is 4.
    for (what, text, input, lead) in [
        ("temor's prompt", "      ", Some(6), Lead::Blank),
        ("a one-line prompt", "~ $ ", Some(4), Lead::Header),
        ("a prompt with text", "~/temor main", Some(5), Lead::Header),
        ("blank, not marked yet", "", None, Lead::Header),
        ("blank, typed on that row", "", Some(4), Lead::Header),
    ] {
        let row = RowText::plain(text, Wrap::Ends);
        assert_eq!(leads_blank(&row, input, 4), lead, "{what}");
    }
}
