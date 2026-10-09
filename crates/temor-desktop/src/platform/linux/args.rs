//! Reading an action's arguments. The router has already checked them against the manifest
//! (type, range, options); this reads them into temor's own types and refuses what a window
//! cannot hold.

use crate::{LineRange, Network, PaneKind, PaneRef, Placement, PortRefusal};
use docket_core::{AppRefusal, Args, FailText, ParamName, Value};
use std::path::PathBuf;

/// The arguments of one call.
pub(super) struct Reader<'a> {
    args: &'a Args,
}

pub(super) fn failed(words: &str) -> AppRefusal {
    AppRefusal::Failed(FailText(words.to_owned()))
}

/// What the window said, as the agent layer's refusal.
pub(super) fn refusal(why: PortRefusal) -> AppRefusal {
    match why {
        PortRefusal::Busy => AppRefusal::Busy,
        PortRefusal::Failed(words) => AppRefusal::Failed(FailText(words)),
        other => AppRefusal::Failed(FailText(other.to_string())),
    }
}

impl<'a> Reader<'a> {
    pub(super) fn new(args: &'a Args) -> Self {
        Self { args }
    }

    fn value(&self, name: &str) -> Option<&'a Value> {
        self.args
            .iter()
            .find(|(key, _)| key.as_str() == name)
            .map(|(_, labelled)| &labelled.value)
    }

    fn missing(name: &str) -> AppRefusal {
        match ParamName::parse(name) {
            Ok(param) => AppRefusal::NeedsParam {
                param,
                options: Vec::new(),
            },
            Err(_) => AppRefusal::Unsupported,
        }
    }

    pub(super) fn int(&self, name: &str) -> Result<Option<i64>, AppRefusal> {
        match self.value(name) {
            None => Ok(None),
            Some(Value::Integer(n)) => Ok(Some(*n)),
            Some(_) => Err(failed("a number was expected")),
        }
    }

    fn unsigned(&self, name: &str) -> Result<Option<u64>, AppRefusal> {
        self.int(name)?
            .map(|n| u64::try_from(n).map_err(|_| failed("a number from zero up was expected")))
            .transpose()
    }

    pub(super) fn text(&self, name: &str) -> Result<Option<&'a str>, AppRefusal> {
        match self.value(name) {
            None => Ok(None),
            Some(Value::Text(text)) => Ok(Some(text.as_str())),
            Some(_) => Err(failed("words were expected")),
        }
    }

    pub(super) fn need_text(&self, name: &str) -> Result<&'a str, AppRefusal> {
        self.text(name)?.ok_or_else(|| Self::missing(name))
    }

    fn choice(&self, name: &str) -> Result<Option<&'a str>, AppRefusal> {
        match self.value(name) {
            None => Ok(None),
            Some(Value::Choice(id)) => Ok(Some(id.as_str())),
            Some(_) => Err(failed("one of the listed options was expected")),
        }
    }

    /// The pane named, or `front` when the call left it out.
    pub(super) fn pane(&self, front: Option<PaneRef>) -> Result<PaneRef, AppRefusal> {
        match self.unsigned("pane")? {
            Some(id) => Ok(PaneRef(id)),
            None => front.ok_or_else(|| failed("no pane is in front")),
        }
    }

    pub(super) fn command(&self) -> Result<crate::CommandRef, AppRefusal> {
        self.unsigned("command")?
            .map(crate::CommandRef)
            .ok_or_else(|| Self::missing("command"))
    }

    pub(super) fn range(&self) -> Result<LineRange, AppRefusal> {
        let line = |name: &str, default: u32| -> Result<u32, AppRefusal> {
            match self.unsigned(name)? {
                None => Ok(default),
                Some(n) => u32::try_from(n).map_err(|_| failed("that line number is too large")),
            }
        };
        Ok(LineRange {
            from: line("from", 0)?,
            count: line("count", 200)?,
        })
    }

    pub(super) fn kind(&self) -> Result<PaneKind, AppRefusal> {
        match self.choice("kind")? {
            None | Some("terminal") => Ok(PaneKind::Terminal),
            Some("agent") => Ok(PaneKind::Agent),
            Some("viewer") => Ok(PaneKind::Viewer),
            Some(_) => Err(AppRefusal::Unsupported),
        }
    }

    pub(super) fn placement(&self) -> Result<Placement, AppRefusal> {
        match self.choice("placement")? {
            None | Some("tab") => Ok(Placement::Tab),
            Some("split_right") => Ok(Placement::SplitRight),
            Some("split_down") => Ok(Placement::SplitDown),
            Some(_) => Err(AppRefusal::Unsupported),
        }
    }

    /// The network a call asks for. Anything but `closed` reads as open, the cautious way.
    pub(super) fn network(&self) -> Result<Network, AppRefusal> {
        match self.choice("network")? {
            None | Some("closed") => Ok(Network::Closed),
            Some(_) => Ok(Network::Open),
        }
    }

    /// A folder, which must be an absolute path.
    pub(super) fn cwd(&self) -> Result<Option<PathBuf>, AppRefusal> {
        match self.text("cwd")? {
            None | Some("") => Ok(None),
            Some(text) if text.starts_with('/') => Ok(Some(PathBuf::from(text))),
            Some(_) => Err(failed("give the folder as an absolute path")),
        }
    }
}
