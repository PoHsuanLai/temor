//! The key file's reading: lines that name no action of temor's are refused with their number.

use crate::settings::keys_file::overrides_of;

#[test]
fn key_lines_apply_and_the_others_are_refused_by_line() {
    let text = "# my keys\n\
                temor.split-right = Primary+Alt+N\n\
                copy = Primary+Hyper+K\n\
                temor.nope = Primary+K\n\
                paste Primary+V\n\
                redo = none\n";
    let (overrides, issues) = overrides_of(text);
    let kept: Vec<String> = overrides.entries().iter().map(|e| e.action.id()).collect();
    assert_eq!(kept, ["temor.split-right", "redo"]);
    let lines: Vec<usize> = issues.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, [3, 4, 5]);
    assert!(issues[1].1.to_string().contains("temor.nope"), "{issues:?}");
}
