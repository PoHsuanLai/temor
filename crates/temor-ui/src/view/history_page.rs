//! The history page (PLAN P23, `temor://history`): every session by day, newest first, or those in
//! one folder (`?in=`), or one session's commands (`/<id>`). Typing in its field searches the
//! commands and their folders instead. A session opens its own page; a live one's commands go to
//! its tab, and a closed one opens again in its folder. Read from the store through the client
//! (`use_live`), so a command that finishes while the page is up appears on it.

use dioxus::prelude::*;
use ds::base::press::Press;
use ds::components::content::avatar::{AvatarFace, AvatarShape, AvatarSize, AvatarTone, PersonHue};
use ds::components::overlays::empty_state::EmptyForm;
use ds::prelude::*;
use ds::style::tokens::control_size::ControlSize;
use temor_client::query::{Commands, History, Sessions};
use temor_core::history::{Zone, by_day, session_title, span_of};
use temor_core::icon::initial_of;
use temor_core::vocab::text::Cwd;
use temor_core::workspace::WorkspaceIn;
use temor_schema::session::{RanCommand, Session, SessionId};
use temor_schema::url::{HistoryScope, TemorUrl};

use super::live::use_live;
use super::tab_glyph::prompt_glyph;
use crate::edge::local_zone::{local_zone, wall_now};
use crate::hub::{Desk, select_tab};

/// How many sessions or commands the page lists.
const LISTED: usize = 300;

/// The page for `scope`.
#[component]
#[allow(non_snake_case)]
pub fn HistoryPage(scope: HistoryScope, desk: Desk) -> Element {
    match scope {
        HistoryScope::Session(id) => rsx! {
            SessionPage { id, desk }
        },
        HistoryScope::All => rsx! {
            SessionsPage { folder: None::<String>, desk }
        },
        HistoryScope::In(folder) => rsx! {
            SessionsPage { folder: Some(folder), desk }
        },
    }
}

/// Every session (in `folder`, when one is given), by day; or, while the field has text, the
/// commands it finds.
#[component]
#[allow(non_snake_case)]
fn SessionsPage(folder: Option<String>, desk: Desk) -> Element {
    // Which sessions run is the hub's: a session that ends redraws the page.
    let _ = (desk.ui)();
    let mut text = use_signal(String::new);
    let within = folder.clone();
    let sessions = use_live(move || Sessions {
        folder: within.clone(),
        before: None,
        limit: LISTED,
    });
    let within = folder.clone();
    let found = use_live(move || History {
        text: Some(text()).filter(|text| !text.trim().is_empty()),
        folder: within.clone(),
        before: None,
        limit: LISTED,
    });
    let zone = local_zone();
    let today = zone.day_of(wall_now());
    let home = desk.hub.home();
    let tokens = folder
        .as_deref()
        .map(|folder| vec![format!("in {}", tidy(folder, &home))])
        .unwrap_or_default();
    let searching = !text().trim().is_empty();
    let body = match searching {
        false => {
            let listed: Vec<Session> = sessions
                .read()
                .value()
                .map(|all| all.as_ref().clone())
                .unwrap_or_default();
            let running = desk.hub.running_sessions();
            let days = by_day(listed, |session| session.started, zone, today);
            rsx! {
                for (heading, run) in days {
                    div { key: "{heading}", class: "temor-history-day",
                        SectionHeader { title: heading.clone() }
                        div { class: "temor-history-list",
                            for session in run {
                                SessionRow {
                                    key: "{session.id}",
                                    running: running.contains(&session.id),
                                    session,
                                    zone,
                                    home: home.clone(),
                                    desk: desk.clone(),
                                }
                            }
                        }
                    }
                }
            }
        }
        true => {
            let listed: Vec<RanCommand> = found
                .read()
                .value()
                .map(|all| all.as_ref().clone())
                .unwrap_or_default();
            commands_by_day(listed, zone, today, Some(&home), &desk)
        }
    };
    let empty = match searching {
        true => found.read().value().is_some_and(|all| all.is_empty()),
        false => sessions.read().value().is_some_and(|all| all.is_empty()),
    };
    rsx! {
        div { class: "temor-history",
            div { class: "temor-history-search",
                SearchField::<()> {
                    label: "Search history",
                    value: text(),
                    placeholder: "Search commands and folders",
                    size: ControlSize::Regular,
                    tokens,
                    oninput: move |typed: String| text.set(typed),
                    onpick: move |()| {},
                }
            }
            if empty {
                EmptyState {
                    form: match searching {
                        true => EmptyForm::NoResults,
                        false => EmptyForm::Empty,
                    },
                    title: match searching {
                        true => "Nothing matches",
                        false => "No history yet",
                    },
                    description: match searching {
                        true => None,
                        false => Some(TextLine::from("Each shell and the commands it runs are kept here.")),
                    },
                }
            }
            {body}
        }
    }
}

/// One session's row: what it ran or where, its folder, and when.
#[component]
#[allow(non_snake_case)]
fn SessionRow(session: Session, running: bool, zone: Zone, home: String, desk: Desk) -> Element {
    let title = session_title(&session, &home);
    let id = session.id;
    rsx! {
        Row {
            leading: letter(&title),
            title: title.clone(),
            detail: Some(TextLine::from(tidy(&session.folder, &home))),
            accessory: Accessory::Text(span_of(&session, running, zone)),
            onclick: move |_: Press| desk.open_page(TemorUrl::History(HistoryScope::Session(id))),
        }
    }
}

/// One session's commands, under what it is and a way back to it: its tab while it runs, or a
/// new tab in its folder once it has ended.
#[component]
#[allow(non_snake_case)]
fn SessionPage(id: SessionId, desk: Desk) -> Element {
    let _ = (desk.ui)();
    let mut text = use_signal(String::new);
    let kept = use_live(move || Commands(id));
    let zone = local_zone();
    let today = zone.day_of(wall_now());
    let home = desk.hub.home();
    let Some((session, commands)) = kept.read().value().and_then(|kept| kept.as_ref().clone())
    else {
        return rsx! {
            div { class: "temor-history",
                EmptyState { form: EmptyForm::NoResults, title: "This session is not in history" }
            }
        };
    };
    let tab = desk.hub.tab_of_session(id);
    let running = tab.is_some();
    let typed = text().to_lowercase();
    let shown: Vec<RanCommand> = commands
        .iter()
        .rev()
        .filter(|ran| typed.is_empty() || ran.command.to_lowercase().contains(&typed))
        .cloned()
        .collect();
    let about = session_about(&session, commands.len(), running, zone, &home);
    let (going, reopening) = (desk.clone(), desk.clone());
    let folder = session.folder.clone();
    rsx! {
        div { class: "temor-history",
            div { class: "temor-history-head",
                div { class: "temor-history-what",
                    span { class: "temor-history-title", "{session_title(&session, &home)}" }
                    span { class: "temor-history-about", "{about}" }
                }
                match tab {
                    Some(tab) => rsx! {
                        Button {
                            label: "Go to Tab".to_owned(),
                            size: ControlSize::Small,
                            title: Some("Bring this session's tab to the front".to_owned()),
                            onclick: move |_| going.act(select_tab(tab)),
                        }
                    },
                    None => rsx! {
                        Button {
                            label: "Reopen".to_owned(),
                            size: ControlSize::Small,
                            title: Some("Open a new tab in this session's folder".to_owned()),
                            onclick: move |_| reopening.act(WorkspaceIn::NewTab {
                                cwd: Some(Cwd(folder.clone())),
                            }),
                        }
                    },
                }
            }
            div { class: "temor-history-search",
                SearchField::<()> {
                    label: "Search this session",
                    value: text(),
                    placeholder: "Search its commands",
                    size: ControlSize::Regular,
                    oninput: move |typed: String| text.set(typed),
                    onpick: move |()| {},
                }
            }
            if shown.is_empty() {
                EmptyState {
                    form: EmptyForm::NoResults,
                    title: match commands.is_empty() {
                        true => "No commands",
                        false => "Nothing matches",
                    },
                }
            }
            {commands_by_day(shown, zone, today, None, &desk)}
        }
    }
}

/// `commands` by day, newest first; each opens its session's page. `home` is given to show
/// each command's folder (a search across sessions).
fn commands_by_day(
    commands: Vec<RanCommand>,
    zone: Zone,
    today: i64,
    home: Option<&str>,
    desk: &Desk,
) -> Element {
    let days = by_day(commands, |ran| ran.started, zone, today);
    rsx! {
        for (heading, run) in days {
            div { key: "{heading}", class: "temor-history-day",
                SectionHeader { title: heading.clone() }
                div { class: "temor-history-list",
                    for ran in run {
                        CommandRow {
                            key: "{ran.session}-{ran.block.0}",
                            folder: home.map(|home| tidy(&ran.folder, home)),
                            ran,
                            zone,
                            desk: desk.clone(),
                        }
                    }
                }
            }
        }
    }
}

/// One command's row: what was typed, where (in a search), whether it failed, and when.
#[component]
#[allow(non_snake_case)]
fn CommandRow(ran: RanCommand, folder: Option<String>, zone: Zone, desk: Desk) -> Element {
    let session = ran.session;
    let when = zone.time_of(ran.started);
    let said = match ran.exit {
        Some(code) if code != 0 => format!("failed  {when}"),
        _ => when,
    };
    rsx! {
        div { class: "temor-history-command",
            Row {
                leading: RowLeading::Source(prompt_glyph(IconSize::Small)),
                title: ran.command.clone(),
                // The command in the code face, and where it ran beside it in a search.
                content: Some(rsx! {
                    span { class: "temor-history-words ds-truncate",
                        span { class: "temor-history-cmd", "{ran.command}" }
                        if let Some(folder) = folder {
                            span { class: "temor-history-where", "{folder}" }
                        }
                    }
                }),
                accessory: Accessory::Text(said),
                onclick: move |_: Press| {
                    desk.open_page(TemorUrl::History(HistoryScope::Session(session)))
                },
            }
        }
    }
}

/// What a session's page says under its name: where, when, how many commands, and whether it
/// has ended.
fn session_about(
    session: &Session,
    commands: usize,
    running: bool,
    zone: Zone,
    home: &str,
) -> String {
    let count = match commands {
        1 => "1 command".to_owned(),
        n => format!("{n} commands"),
    };
    let mut parts = vec![
        tidy(&session.folder, home),
        span_of(session, running, zone),
        count,
    ];
    if !running {
        parts.push("closed".to_owned());
    }
    parts.join(" · ")
}

/// A letter tile for `title`, as a row's letter is; the home folder (`~`) is H.
fn letter(title: &str) -> RowLeading {
    let initial = match title {
        "~" => 'H',
        title => initial_of(title),
    };
    RowLeading::Avatar(AvatarFace {
        initial,
        size: AvatarSize::Size16,
        tone: AvatarTone::Person(PersonHue::of(title)),
        shape: AvatarShape::Square,
    })
}

/// `path` with the home folder written as `~`.
fn tidy(path: &str, home: &str) -> String {
    match path.strip_prefix(home) {
        Some("") => "~".to_owned(),
        Some(rest) if rest.starts_with('/') && !home.is_empty() => format!("~{rest}"),
        _ => path.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use temor_schema::session::At;
    use temor_schema::url::Host;

    use super::*;

    #[test]
    fn a_sessions_page_says_where_when_how_many_and_whether_it_ended() {
        let session = Session {
            id: SessionId::from_bits(1),
            host: Host::Local,
            folder: "/Users/po/ml".to_owned(),
            program: None,
            started: At(0),
            ended: Some(At(60 * 60_000)),
        };
        assert_eq!(
            session_about(&session, 3, false, Zone(0), "/Users/po"),
            "~/ml · 00:00 – 01:00 · 3 commands · closed"
        );
        assert_eq!(
            session_about(&session, 1, true, Zone(0), "/Users/po"),
            "~/ml · 00:00 – now · 1 command"
        );
    }
}
