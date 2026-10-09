//! The line walker: builds the blocks of one config (following `Include` in place, as
//! OpenSSH does) and records what it had to ignore. Nothing here fails; every problem is an
//! issue.

use std::path::{Path, PathBuf};

use super::include::{ConfigFiles, ConfigPaths, expand_include};
use super::lex::{Line, lex_line};
use super::model::{
    ConfigIssue, HostName, IdentityFile, IssueReason, LineNo, Port, ProxyJump, UserName,
};
use super::pattern::{HostPattern, Scope};

/// OpenSSH stops following includes 16 levels deep.
const MAX_INCLUDE_DEPTH: usize = 16;
/// Total file reads for one config, across every include, so a glob that includes itself
/// cannot fan out.
const MAX_INCLUDE_READS: usize = 256;
/// Total bytes read for one config.
const MAX_INCLUDE_BYTES: usize = 1024 * 1024;

/// Where a block's `Host` or `Match` line was.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Origin {
    pub file: PathBuf,
    pub line: LineNo,
}

/// A setting with a value that validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Setting {
    HostName(HostName),
    User(UserName),
    Port(Port),
    IdentityFile(IdentityFile),
    ProxyJump(ProxyJump),
}

/// A `Host`, `Match` or global section and its settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Block {
    pub scope: Scope,
    pub settings: Vec<Setting>,
    pub origin: Origin,
}

/// Walks config text into blocks. The current block carries across `Include`, as the text
/// would if it were pasted in.
pub(super) struct Walker<'a> {
    files: &'a dyn ConfigFiles,
    paths: &'a ConfigPaths,
    blocks: Vec<Block>,
    issues: Vec<ConfigIssue>,
    current: usize,
    reads: usize,
    bytes: usize,
}

impl<'a> Walker<'a> {
    pub(super) fn new(files: &'a dyn ConfigFiles, paths: &'a ConfigPaths) -> Self {
        let global = Block {
            scope: Scope::Global,
            settings: Vec::new(),
            origin: Origin {
                file: paths.config.clone(),
                line: LineNo(0),
            },
        };
        Self {
            files,
            paths,
            blocks: vec![global],
            issues: Vec::new(),
            current: 0,
            reads: 0,
            bytes: 0,
        }
    }

    pub(super) fn into_parts(self) -> (Vec<Block>, Vec<ConfigIssue>) {
        (self.blocks, self.issues)
    }

    /// Reads and walks one file. A missing file is silent; an unreadable one is an issue.
    pub(super) fn load_file(&mut self, path: &Path, depth: usize) {
        if self.reads >= MAX_INCLUDE_READS {
            self.issue(path, LineNo(0), IssueReason::IncludeBudgetExceeded);
            return;
        }
        self.reads += 1;
        match self.files.read(path) {
            Ok(text) => {
                self.bytes = self.bytes.saturating_add(text.len());
                if self.bytes > MAX_INCLUDE_BYTES {
                    self.issue(path, LineNo(0), IssueReason::IncludeBudgetExceeded);
                    return;
                }
                self.walk(&text, path, depth);
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => self.issue(
                path,
                LineNo(0),
                IssueReason::IncludeUnreadable {
                    path: path.to_path_buf(),
                },
            ),
        }
    }

    fn walk(&mut self, text: &str, path: &Path, depth: usize) {
        for (index, raw) in text.lines().enumerate() {
            let line_no = LineNo(u32::try_from(index + 1).unwrap_or(u32::MAX));
            match lex_line(raw) {
                Ok(Some(line)) => self.directive(line, path, line_no, depth),
                Ok(None) => {}
                Err(_) => self.issue(path, line_no, IssueReason::UnterminatedQuote),
            }
        }
    }

    fn directive(&mut self, line: Line, path: &Path, line_no: LineNo, depth: usize) {
        match line.keyword.as_str() {
            "host" => self.host(line.args, path, line_no),
            "match" => {
                self.push(Scope::Skipped, path, line_no);
                self.issue(path, line_no, IssueReason::MatchSkipped);
            }
            "include" => self.include(&line.args, path, line_no, depth),
            "hostname" | "user" | "port" | "identityfile" | "proxyjump" => {
                self.setting(&line.keyword, &line.args, path, line_no);
            }
            _ => {}
        }
    }

    fn host(&mut self, args: Vec<String>, path: &Path, line_no: LineNo) {
        if args.is_empty() {
            self.issue(path, line_no, missing("Host"));
        }
        let patterns = args.iter().map(|arg| HostPattern::parse(arg)).collect();
        self.push(Scope::Hosts(patterns), path, line_no);
    }

    fn push(&mut self, scope: Scope, path: &Path, line_no: LineNo) {
        self.blocks.push(Block {
            scope,
            settings: Vec::new(),
            origin: Origin {
                file: path.to_path_buf(),
                line: line_no,
            },
        });
        self.current = self.blocks.len() - 1;
    }

    fn include(&mut self, args: &[String], path: &Path, line_no: LineNo, depth: usize) {
        if args.is_empty() {
            self.issue(path, line_no, missing("Include"));
            return;
        }
        if depth >= MAX_INCLUDE_DEPTH {
            self.issue(path, line_no, IssueReason::IncludeTooDeep);
            return;
        }
        for arg in args {
            for included in expand_include(self.paths, self.files, arg) {
                if self.reads >= MAX_INCLUDE_READS || self.bytes > MAX_INCLUDE_BYTES {
                    // Reported once here, not once per remaining glob match.
                    self.issue(path, line_no, IssueReason::IncludeBudgetExceeded);
                    return;
                }
                self.load_file(&included, depth + 1);
            }
        }
    }

    fn setting(&mut self, keyword: &str, args: &[String], path: &Path, line_no: LineNo) {
        let canonical = canonical_keyword(keyword);
        let value = match args {
            [] => return self.issue(path, line_no, missing(canonical)),
            [one] => one,
            many => {
                let value = many.join(" ");
                return self.issue(path, line_no, bad(canonical, value));
            }
        };
        let parsed = match keyword {
            "hostname" => HostName::new(value).map(Setting::HostName),
            "user" => UserName::new(value).map(Setting::User),
            "port" => Port::parse(value).map(Setting::Port),
            "identityfile" => IdentityFile::new(value).map(Setting::IdentityFile),
            _ => ProxyJump::new(value).map(Setting::ProxyJump),
        };
        match parsed {
            Some(setting) => self.blocks[self.current].settings.push(setting),
            None => self.issue(path, line_no, bad(canonical, value.clone())),
        }
    }

    fn issue(&mut self, path: &Path, line: LineNo, reason: IssueReason) {
        self.issues.push(ConfigIssue {
            file: path.to_path_buf(),
            line,
            reason,
        });
    }
}

fn missing(keyword: &str) -> IssueReason {
    IssueReason::MissingValue {
        keyword: keyword.to_owned(),
    }
}

fn bad(keyword: &str, value: String) -> IssueReason {
    IssueReason::BadValue {
        keyword: keyword.to_owned(),
        value,
    }
}

/// The keyword as OpenSSH writes it, for messages.
fn canonical_keyword(lower: &str) -> &'static str {
    match lower {
        "hostname" => "HostName",
        "user" => "User",
        "port" => "Port",
        "identityfile" => "IdentityFile",
        _ => "ProxyJump",
    }
}
