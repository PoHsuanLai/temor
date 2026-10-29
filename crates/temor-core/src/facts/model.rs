//! The facts and their parts.

/// The name of a Python virtualenv: the last part of its folder.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnvName(pub String);

impl EnvName {
    /// The name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A Node version as the project asks for it (`20`, `v18.19.0`, `>=18`), cut to be a chip.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NodeVersion(pub String);

impl NodeVersion {
    /// The version text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A Python, Ruby or Go version from `.tool-versions`, cut to be a chip.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ToolVersion(pub String);

impl ToolVersion {
    /// The version text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What is known about a pane's folder. A fact that could not be read is `None`; none of them is
/// ever an error the person sees.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Facts {
    pub venv: Option<EnvName>,
    pub node: Option<NodeVersion>,
    pub python: Option<ToolVersion>,
    pub ruby: Option<ToolVersion>,
    pub go: Option<ToolVersion>,
}

impl Facts {
    /// Nothing is known, so nothing is shown.
    pub fn is_empty(&self) -> bool {
        self.venv.is_none()
            && self.node.is_none()
            && self.python.is_none()
            && self.ruby.is_none()
            && self.go.is_none()
    }
}
