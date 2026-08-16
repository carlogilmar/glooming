//! Source parsing. Rust owns the syntax tree; the frontend only ever sees an
//! [`Outline`]. Adding a language means adding a module here that produces the
//! same shape — nothing downstream changes.

pub mod elixir;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

/// One `name/arity` entry. Multiple clauses of the same function collapse into
/// a single `FnInfo` whose `clauses` counts them — Elixir routinely spreads one
/// function across several `def`s, and the reader wants one row, not four.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FnInfo {
    pub name: String,
    pub arity: u8,
    /// Lowest callable arity. Differs from `arity` only when the definition has
    /// default arguments (`def f(a, b \\ nil)` → `min_arity` 1, `arity` 2),
    /// which the UI renders as `f/1..2`.
    pub min_arity: u8,
    pub visibility: Visibility,
    /// 1-based line of the first clause.
    pub line: u32,
    /// 1-based line of that clause's matching `end` (== `line` for one-liners).
    pub end_line: u32,
    pub clauses: u8,
    /// `@doc` text attached to the first clause, if any.
    pub doc: Option<String>,
}

impl FnInfo {
    /// `create_user/1` — the key prose is stored against, and the identity used
    /// when reconciling a doc with re-parsed source.
    pub fn signature(&self) -> String {
        format!("{}/{}", self.name, self.arity)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleInfo {
    pub name: String,
    pub line: u32,
    pub doc: Option<String>,
    pub functions: Vec<FnInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Outline {
    pub lang: String,
    pub modules: Vec<ModuleInfo>,
}

impl Outline {
    pub fn function_count(&self) -> usize {
        self.modules.iter().map(|m| m.functions.len()).sum()
    }
}

/// Language dispatch by file extension. Unknown extensions are not an error —
/// the file still opens, it just has no outline to seed from.
pub fn lang_for_path(path: &str) -> Option<&'static str> {
    match path.rsplit('.').next()? {
        "ex" | "exs" => Some("elixir"),
        _ => None,
    }
}

pub fn parse(source: &str, lang: &str) -> crate::error::AppResult<Outline> {
    match lang {
        "elixir" => elixir::parse(source),
        other => Err(crate::error::AppError::Parse(format!(
            "no parser for language {other}"
        ))),
    }
}
