//! `shell.bin`: OSC 7, 133 A/B/C/D with an exit code, 0, 7501, 9, 777, 9;4, 52 and 8, then BEL.

use temor_vt::{
    ClipboardGrant, ClipboardLocation, ExitCode, ExitStatus, GridPoint, Hyperlink, Notification,
    Percent, PointSpace, ProgramState, ProgramWait, Progress, ProgressState, PromptEvent,
    PromptKind, SemanticMark, VtEvent,
};

use crate::support::{drain, fixture, row_cells, term};

#[test]
fn shell_integration_and_reports_arrive_as_typed_events_in_order() {
    let (mut t, rx) = term(80, 10);
    t.write(&fixture("shell.bin"));
    let mut events = drain(&rx).into_iter();
    let mut next = || events.next().expect("another event");

    assert_eq!(next(), VtEvent::Cwd("file://host/home/me".into()));
    assert_eq!(
        next(),
        VtEvent::Prompt(PromptEvent::PromptStart(PromptKind::Primary))
    );
    assert_eq!(next(), VtEvent::Prompt(PromptEvent::InputStart));
    assert_eq!(
        next(),
        VtEvent::Prompt(PromptEvent::OutputStart {
            command: "false".into()
        })
    );
    assert_eq!(
        next(),
        VtEvent::Prompt(PromptEvent::CommandEnd {
            exit: ExitStatus::Code(ExitCode(1)),
            error: String::new()
        })
    );
    let VtEvent::ProgramStatus(blocked) = next() else {
        panic!("expected a program status")
    };
    assert_eq!(
        (
            blocked.state,
            blocked.wait,
            blocked.app.as_str(),
            blocked.title.as_str(),
            blocked.message.as_str()
        ),
        (
            ProgramState::Blocked,
            ProgramWait::Permission,
            "terraform",
            "Deploy",
            "Apply?"
        )
    );
    let VtEvent::ProgramStatus(working) = next() else {
        panic!("expected a program status")
    };
    assert_eq!(
        (working.state, working.percent, working.app.as_str()),
        (ProgramState::Working, Percent::new(30), "cargo")
    );
    assert_eq!(
        next(),
        VtEvent::Notification(Notification {
            title: String::new(),
            body: "plain body".into()
        })
    );
    assert_eq!(
        next(),
        VtEvent::Notification(Notification {
            title: "Title".into(),
            body: "Body".into()
        })
    );
    assert_eq!(
        next(),
        VtEvent::Progress(Progress {
            state: ProgressState::Set,
            percent: Percent::new(42)
        })
    );
    assert_eq!(
        next(),
        VtEvent::Progress(Progress {
            state: ProgressState::Remove,
            percent: None
        })
    );
    let VtEvent::Clipboard { write } = next() else {
        panic!("expected a clipboard write")
    };
    assert_eq!(
        (write.location, write.grant),
        (ClipboardLocation::Standard, ClipboardGrant::Ask)
    );
    assert_eq!(
        write
            .items
            .iter()
            .map(|i| i.data.as_slice())
            .collect::<Vec<_>>(),
        [b"copied".as_slice()]
    );
    assert_eq!(next(), VtEvent::Bell);
    // The title is read once the write has returned, so it comes last.
    assert_eq!(next(), VtEvent::Title("my title".into()));
    assert_eq!(events.next(), None);

    assert_eq!(t.title(), "my title");
    assert_eq!(t.pwd(), "file://host/home/me");
}

#[test]
fn prompt_marks_and_hyperlinks_show_on_cells() {
    let (mut t, _rx) = term(80, 10);
    t.write(&fixture("shell.bin"));
    let prompt_row = row_cells(&mut t, 0);
    let marks: Vec<SemanticMark> = prompt_row.iter().take(7).map(|c| c.semantic).collect();
    assert_eq!(&marks[..2], [SemanticMark::Prompt, SemanticMark::Prompt]);
    assert_eq!(marks[2], SemanticMark::Input, "`false` is typed input");

    let link_row = row_cells(&mut t, 1);
    let linked: String = link_row
        .iter()
        .filter(|c| c.hyperlink == Hyperlink::Linked)
        .map(|c| c.text.as_str())
        .collect();
    assert_eq!(linked, "link");
    let at = GridPoint::new(PointSpace::Viewport, 0, 1);
    assert_eq!(t.hyperlink_at(at), Ok(Some("https://example.org/x".into())));
    assert_eq!(
        t.hyperlink_at(GridPoint::new(PointSpace::Viewport, 6, 1)),
        Ok(None)
    );
}
