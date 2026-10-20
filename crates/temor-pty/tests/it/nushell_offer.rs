//! The shell offer's two steps in a real nushell (PLAN P17): the one-line question names the
//! shell, and the text written for nushell then loads without a parse error. The terminal
//! answers nushell's cursor-position queries (`ESC [ 6 n`), which it waits for before a prompt.

use temor_pty::prepare_shell;
use temor_shell::{ChildEnv, Shell, ShellStart, integration_scripts};
use temor_ssh::{InnerLoadLine, InnerShell, LoadToken, probe_line};

use crate::support::{Scratch, Term, find, program_or_skip, pty_unsupported, setsid, size};

#[test]
fn nushell_names_itself_and_loads_the_text_written_for_it_without_complaint() {
    if pty_unsupported("nushell_names_itself_and_loads_the_text_written_for_it_without_complaint") {
        return;
    }
    let Some(nu) = program_or_skip("nu", "nushell_offer") else {
        return;
    };
    let scratch = Scratch::new();
    let spec = prepare_shell(
        &Shell::at(nu),
        ShellStart::Interactive,
        scratch.contract_env(&ChildEnv::new()),
        &scratch.runtime,
        scratch.cwd.clone(),
        size(),
    )
    .expect("prepare shell");
    let mut term = Term::spawn(&spec, &setsid());
    assert!(
        term.read_until_has(0, "\x1b]133;B"),
        "no prompt mark: {:?}",
        term.text()
    );

    let start = term.out.len();
    term.send(&probe_line().replace('\n', "\r"));
    // The answer is `nu` from `/proc`, or a path ending in `nu` where `ps` gives one (macOS).
    let named = |out: &[u8]| {
        let seen = &out[start..];
        find(seen, b"temor-shell:nu").is_some() || find(seen, b"/nu\r").is_some()
    };
    assert!(
        term.read_until(named),
        "the shell did not name itself: {:?}",
        term.text()
    );

    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.subsec_nanos());
    let token = LoadToken::from_words((u64::from(std::process::id()) << 32) | u64::from(nanos), 7);
    let text = InnerLoadLine::build_for(InnerShell::Nu, &integration_scripts(), &token)
        .expect("build")
        .as_str()
        .replace('\n', "\r");
    let start = term.out.len();
    // A few bytes at a time, as over a slow link: nushell reads each piece as it comes.
    for piece in text.as_bytes().chunks(64) {
        term.send(std::str::from_utf8(piece).expect("ascii"));
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(
        term.read_until_has(start, "\x1b]133;D;0"),
        "the load did not finish: {:?}",
        term.text()
    );
    let ended = term.out.len();
    assert!(
        term.read_until_has(ended, "\x1b]133;B"),
        "no prompt after the load: {:?}",
        term.text()
    );
    term.send("print ('loaded-' + 'ok')\r");
    assert!(term.read_until_has(start, "loaded-ok"), "{:?}", term.text());
    let shown = String::from_utf8_lossy(&term.out[start..]).into_owned();
    // One paste is drawn a few times (about 6 KB here); typed as a quoted program over many
    // lines, nushell redrew the whole quote for every line (megabytes for a few KB of text).
    assert!(
        shown.len() < 64 * 1024,
        "nushell drew {} bytes for {} typed",
        shown.len(),
        text.len()
    );
    for complaint in ["Error", "parse", "not found", "Extra positional"] {
        assert!(!shown.contains(complaint), "nushell complained: {shown:?}");
    }
}
