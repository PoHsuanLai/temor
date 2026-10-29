//! temor's window layout (sans I/O): the workspace of tabs and their split trees, Spaces, the quick terminal, the command bar's ranking and a row's stage.

pub mod bar;
pub mod quick;
pub mod space;
pub mod split;
pub mod stage;
pub mod workspace;

#[cfg(test)]
mod test_support;
