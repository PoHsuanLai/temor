//! Unit tests for the settings text: good lines apply, bad lines are reported by line number.

use super::{NotifyAfter, TerminalSettings, parse_settings};

#[test]
fn settings_text_applies_good_lines_and_reports_bad_ones_by_line() {
    let text = "# comment\n\
                font.size = 15\n\
                temor.split-right = Primary+Alt+N\n\
                font.size = huge\n\
                temor.nope = Primary+K\n\
                keys.copy = ctrl+c\n\
                copy = Primary+Hyper+K\n\
                bogus = 1\n\
                scrollback.lines = 500\n";
    let parsed = parse_settings(text);
    assert_eq!(parsed.settings.font_size.points(), 15.0);
    assert_eq!(parsed.settings.scrollback.lines(), 500);
    assert_eq!(parsed.settings.line_height.percent(), 100);
    let lines: Vec<usize> = parsed.issues.iter().map(|(line, _)| *line).collect();
    assert_eq!(lines, vec![4, 5, 6, 7, 8]);
    let old = parsed
        .issues
        .iter()
        .find(|(line, _)| *line == 6)
        .expect("line 6");
    assert!(
        old.1.to_string().contains("name the action itself"),
        "an old `keys.` line says what to write instead: {}",
        old.1
    );
    let kept: Vec<String> = parsed
        .settings
        .keys
        .entries()
        .iter()
        .map(|entry| entry.action.id())
        .collect();
    assert_eq!(kept, vec!["temor.split-right".to_owned()]);
}

#[test]
fn settings_values_have_limits() {
    assert!(parse_settings("font.size = 5").issues.len() == 1);
    assert!(parse_settings("font.size = 72").issues.is_empty());
    assert!(parse_settings("scrollback.lines = 1000001").issues.len() == 1);
    assert!(parse_settings("cursor.style = blink").issues.len() == 1);
    assert_eq!(parse_settings("").settings.scrollback.lines(), 10_000);
}

#[test]
fn line_height_is_a_percent_from_80_to_200() {
    // (line, the percent it gives, refused)
    for (line, want, refused) in [
        ("", 100, false),
        ("font.line_height = 120", 120, false),
        ("font.line_height = 125%", 125, false),
        ("font.line_height = 79", 100, true),
        ("font.line_height = 201", 100, true),
        ("font.line_height = tall", 100, true),
    ] {
        let parsed = parse_settings(line);
        assert_eq!(parsed.settings.line_height.percent(), want, "{line:?}");
        assert_eq!(!parsed.issues.is_empty(), refused, "{line:?}");
    }
}

#[test]
fn cursor_blink_reads_on_and_off() {
    use super::CursorBlinking;
    // (line, what it gives, refused)
    for (line, want, refused) in [
        ("", CursorBlinking::On, false),
        ("cursor.blink = off", CursorBlinking::Off, false),
        ("cursor.blink = on", CursorBlinking::On, false),
        ("cursor.blink = yes", CursorBlinking::On, true),
    ] {
        let parsed = parse_settings(line);
        assert_eq!(parsed.settings.cursor_blink, want, "{line}");
        assert_eq!(parsed.issues.len() == 1, refused, "{line}");
    }
}

#[test]
fn notify_after_reads_seconds_and_off() {
    assert_eq!(
        TerminalSettings::default().notify_after,
        NotifyAfter::Seconds(10)
    );
    let parsed = parse_settings("notify.after = 30\n");
    assert_eq!(parsed.settings.notify_after, NotifyAfter::Seconds(30));
    assert_eq!(
        parse_settings("notify.after = off").settings.notify_after,
        NotifyAfter::Off
    );
    assert_eq!(
        parse_settings("notify.after = 0").settings.notify_after,
        NotifyAfter::Off
    );
    assert_eq!(parse_settings("notify.after = soon").issues.len(), 1);
}

#[test]
fn offer_lines_collect_in_order_and_refuse_empty() {
    let parsed = parse_settings(
        "offer.add = kubectl  debug -it\noffer.add = mosh\noffer.remove = su\noffer.add =\n",
    );
    assert_eq!(parsed.settings.offer_add, vec!["kubectl debug -it", "mosh"]);
    assert_eq!(parsed.settings.offer_remove, vec!["su"]);
    assert_eq!(parsed.issues.len(), 1);
}
