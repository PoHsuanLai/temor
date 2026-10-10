use super::*;
use crate::vocab::ai::AiSwitch;
use crate::vocab::ids::Bytes;
use crate::vocab::text::{CommandText, Cwd, RelPath};

fn sources() -> ContextSources {
    ContextSources {
        request: "why did this fail".into(),
        cwd: Some(Cwd("/home/p/proj".into())),
        shell: Some("bash".into()),
        command: Some(CommandText("cargo build".into())),
        output: Some((1..=100).map(|n| format!("line {n}\n")).collect()),
        selection: Some("sel".into()),
        files: vec![RelPath("src/a.rs".into()), RelPath("src/b.rs".into())],
    }
}

fn kinds(card: &ContextCard) -> Vec<ChipKind> {
    card.chips.iter().map(|c| c.kind).collect()
}

#[test]
fn each_entry_point_has_fixed_defaults_and_missing_pieces_are_not_guessed() {
    use ChipKind as K;
    let full = sources();
    let bare = ContextSources {
        request: "x".into(),
        ..ContextSources::default()
    };
    #[rustfmt::skip]
    let cases = [
        ("failed block, all there", EntryPoint::FailedBlock, &full, vec![K::Request, K::Cwd, K::Command, K::Output]),
        ("suggest a command", EntryPoint::SuggestCommand, &full, vec![K::Request, K::Cwd, K::Shell]),
        ("ask about a tab: one chip per file", EntryPoint::AskAboutTab, &full, vec![K::Request, K::Cwd, K::Selection, K::FileName, K::FileName]),
        ("nothing but the request", EntryPoint::FailedBlock, &bare, vec![K::Request]),
    ];
    for (name, entry, from, want) in cases {
        assert_eq!(kinds(&build_card(entry, from)), want, "{name}");
    }
    let card = build_card(EntryPoint::FailedBlock, &full);
    let output = &card.chips[3];
    assert_eq!(output.text.lines().count(), OUTPUT_LINES);
    assert!(
        output.text.ends_with("line 100"),
        "the last lines, not the first"
    );
    assert_eq!(output.size(), Bytes(output.text.len() as u64));
}

#[test]
fn removing_sending_and_the_ai_switch() {
    let card = build_card(EntryPoint::FailedBlock, &sources());
    let phase = ContextPhase::Editing(card.clone());
    let (phase, _) = context_step(phase, ContextIn::Remove(ChipId(0)));
    assert_eq!(
        phase,
        ContextPhase::Editing(card.clone()),
        "the request cannot be removed"
    );

    let (phase, _) = context_step(phase, ContextIn::Remove(ChipId(3)));
    let (phase, _) = context_step(phase, ContextIn::Remove(ChipId(1)));
    let (phase, _) = context_step(phase, ContextIn::AddBack(ChipId(1)));
    let ContextPhase::Editing(shown) = &phase else {
        panic!("still editing");
    };
    assert_eq!(
        kinds(shown),
        [ChipKind::Request, ChipKind::Cwd, ChipKind::Command]
    );

    let (phase, out) = context_step(phase, ContextIn::Send(AiSwitch::Off));
    assert!(
        out.is_empty() && matches!(phase, ContextPhase::Editing(_)),
        "AI off sends nothing"
    );

    let (phase, out) = context_step(phase, ContextIn::Send(AiSwitch::On));
    let [ContextOut::Send(list)] = out.as_slice() else {
        panic!("one send, got {out:?}");
    };
    assert_eq!(list.chips.len(), 3);
    assert_eq!(list.total, Bytes(list.chips.iter().map(|c| c.size.0).sum()));
    assert_eq!(phase, ContextPhase::Sent(list.clone()));

    let (after, out) = context_step(phase.clone(), ContextIn::Cancel);
    assert!(out.is_empty() && after == phase, "a sent card is final");
}
