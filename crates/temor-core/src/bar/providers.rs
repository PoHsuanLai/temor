//! The bar's local providers as plain data: what the window knows, listed as [`Candidate`]s for
//! [`rank`](super::rank). Nothing here reads a file or a clock; SSH hosts come as a list (the
//! edge parses `~/.ssh/config`), and directory use comes with its counts.

use super::frecency::{UnixSecs, Visits};
use super::model::{Candidate, LayoutName, Target};
use crate::space::{Folder, Project, Tab};
use crate::vocab::text::{CommandText, Cwd, HostAlias};
use ds_style::space::list::{Space, SpaceId};

/// A Space as the bar lists it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpaceEntry {
    /// Its id.
    pub id: SpaceId,
    /// What the switcher calls it.
    pub name: String,
}

/// A folder the person opened tabs in, with how much.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectoryUse {
    /// The folder.
    pub cwd: Cwd,
    /// How often and how lately.
    pub visits: Visits,
}

/// Everything the bar can offer for a query, as the window holds it now.
#[derive(Debug, Clone, Copy)]
pub struct Sources<'a> {
    /// The Spaces.
    pub spaces: &'a [SpaceEntry],
    /// The tabs of the Space in front.
    pub tabs: &'a [Tab],
    /// The sidebar's folders.
    pub folders: &'a [Folder],
    /// The SSH hosts, as the edge listed them.
    pub hosts: &'a [HostAlias],
    /// The saved layouts.
    pub layouts: &'a [LayoutName],
    /// Commands run lately, newest first.
    pub recent: &'a [CommandText],
    /// Folders with their use.
    pub directories: &'a [DirectoryUse],
    /// The time for the frecency scores.
    pub now: UnixSecs,
}

/// The Spaces of the kit's list as bar entries.
pub fn space_entries(list: &[Space<Project>]) -> Vec<SpaceEntry> {
    list.iter()
        .map(|space| SpaceEntry {
            id: space.id,
            name: space.name.clone(),
        })
        .collect()
}

/// The candidates, kinds in the order §7 lists them. Rows with no text to match are left out, as
/// are recent commands that could not be typed into a prompt as they are (a control character).
pub fn candidates(sources: &Sources<'_>) -> Vec<Candidate> {
    let named = |title: &str, target| {
        (!title.trim().is_empty()).then(|| Candidate {
            title: title.to_owned(),
            target,
        })
    };
    let mut out = Vec::new();
    let spaces = sources
        .spaces
        .iter()
        .map(|s| named(&s.name, Target::Space(s.id)));
    let tabs = sources
        .tabs
        .iter()
        .map(|t| named(t.title.as_str(), Target::Tab(t.id)));
    let folders = sources
        .folders
        .iter()
        .map(|f| named(f.title.as_str(), Target::Folder(f.id)));
    let hosts = sources
        .hosts
        .iter()
        .map(|h| named(h.as_str(), Target::Host(h.clone())));
    let layouts = sources
        .layouts
        .iter()
        .map(|l| named(&l.0, Target::Layout(l.clone())));
    let recent = sources
        .recent
        .iter()
        .filter(|c| c.typable())
        .map(|c| named(c.as_str(), Target::RecentCommand(c.clone())));
    let directories = sources.directories.iter().map(|d| {
        let frecency = d.visits.frecency(sources.now);
        named(
            d.cwd.as_str(),
            Target::Directory {
                cwd: d.cwd.clone(),
                frecency,
            },
        )
    });
    out.extend(spaces.flatten());
    out.extend(tabs.flatten());
    out.extend(folders.flatten());
    out.extend(hosts.flatten());
    out.extend(layouts.flatten());
    out.extend(recent.flatten());
    out.extend(directories.flatten());
    out
}
