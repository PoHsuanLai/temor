//! The list of computers to connect to (PLAN §9b, D22): the hosts of the person's SSH settings
//! and their tailnet machines, each machine once, grouped, with offline ones dimmed. Pressing a
//! row opens a new tab on it. The command bar shows the same rows through `HostRows`.

use dioxus::prelude::*;
use ds::base::press::Press;
use ds::base::vocab::{Availability, RowState};
use ds::prelude::*;
use temor_ssh::{HostGroup, HostRow, Presence};

use crate::hub::{Desk, HostRows};

/// The heading each group is listed under.
pub fn group_title(group: HostGroup) -> &'static str {
    match group {
        HostGroup::SshConfig => "Your hosts",
        HostGroup::YourComputers => "Your computers",
        _ => "More computers",
    }
}

/// What a row says under its name: that a tailnet machine is offline, or where an SSH host goes.
pub fn row_detail(row: &HostRow) -> Option<String> {
    match (row.presence(), row) {
        (Some(Presence::Offline), _) => Some("Offline".to_owned()),
        (_, HostRow::Configured { entry, .. }) => Some(entry.hostname.as_str().to_owned()),
        _ => None,
    }
}

/// How a row takes input: an offline machine is shown dimmed and is not pressed.
pub fn row_availability(row: &HostRow) -> Availability {
    match row.presence() {
        Some(Presence::Offline) => Availability::Disabled,
        _ => Availability::Enabled,
    }
}

/// The computers, by group.
#[component]
#[allow(non_snake_case)]
pub fn HostList(desk: Desk) -> Element {
    let _ = (desk.ui)();
    let rows = desk.hub.host_rows();
    let mut items: Vec<ListItem<String>> = Vec::new();
    for group in [HostGroup::SshConfig, HostGroup::YourComputers] {
        let members: Vec<&HostRow> = rows.iter().filter(|row| row.group() == group).collect();
        if members.is_empty() {
            continue;
        }
        let title = group_title(group).to_owned();
        items.push(ListItem::heading(
            format!("group:{title}"),
            rsx! { SectionHeader { title: title.clone() } },
        ));
        for row in members {
            let name = row.shown().to_owned();
            let state = RowState {
                availability: row_availability(row),
                ..RowState::default()
            };
            let detail = row_detail(row).map(TextLine::from);
            let opening = desk.clone();
            let target = row.clone();
            let content = rsx! {
                Row {
                    title: name.clone(),
                    detail,
                    state,
                    onclick: move |_: Press| opening.open_host(&target),
                }
            };
            items.push(
                ListItem::row(format!("host:{name}"), name, content).with(row_availability(row)),
            );
        }
    }
    if items.is_empty() {
        return rsx! {
            Label { text: "No computers yet. Add one to your SSH settings, or sign in to your tailnet.".to_owned() }
        };
    }
    rsx! {
        List::<String> { label: "Computers".to_owned(), items }
    }
}
