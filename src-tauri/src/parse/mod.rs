//! Source parsing. Rust owns the syntax tree; the frontend only ever sees an
//! [`Outline`]. Adding a language means adding a module here that produces the
//! same shape — nothing downstream changes.

pub mod elixir;
pub mod kinds;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Visibility {
    Public,
    Private,
}

/// An inclusive 1-based line span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Range {
    pub start: u32,
    pub end: u32,
}

impl Range {
    pub fn of(node: tree_sitter::Node) -> Self {
        Range {
            start: node.start_position().row as u32 + 1,
            end: node.end_position().row as u32 + 1,
        }
    }
}

/// One `name/arity` entry. Multiple clauses of the same function collapse into
/// a single `FnInfo` whose `clauses` counts them — Elixir routinely spreads one
/// function across several `def`s, and the reader wants one row, not four.
/// Every clause's span is kept in `clause_ranges` so selecting the row can
/// highlight all of them, not just the first.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
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
    /// Every clause of this function, in source order.
    pub clause_ranges: Vec<Range>,
    /// `@doc` text attached to the first clause, if any.
    pub doc: Option<String>,
    /// Where that `@doc` sits, so the code pane can style it.
    pub doc_range: Option<Range>,
    /// Where the `@spec` sits — highlighted alongside the function, in its own
    /// color, because the signature is half of what you're reading.
    pub spec_range: Option<Range>,
}

impl FnInfo {
    /// `create_user/1` — the key prose is stored against, and the identity used
    /// when matching a doc's prose to a function.
    pub fn signature(&self) -> String {
        format!("{}/{}", self.name, self.arity)
    }
}

/// How far outside the module a dependency lives. The split that matters is
/// `App`: nobody cares that you call `Enum.map/2`, but that `Accounts` reaches
/// into `Billing` is an architectural fact.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DepKind {
    /// Shares the current module's root namespace.
    App,
    /// A dependency — Ecto, Phoenix, anything third-party.
    Lib,
    /// Elixir's own standard library.
    Std,
}

impl DepKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            DepKind::App => "app",
            DepKind::Lib => "lib",
            DepKind::Std => "std",
        }
    }
}

/// One function of an external module that this file actually calls, and the
/// local functions doing the calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteFn {
    /// `insert/1`, or `%User{}` for a struct literal.
    pub name: String,
    /// Local `name/arity` signatures, in source order, deduplicated.
    pub callers: Vec<String>,
}

/// An external module this file reaches, and the surface of it being used.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Dep {
    pub module: String,
    pub kind: DepKind,
    pub functions: Vec<RemoteFn>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModuleInfo {
    pub name: String,
    pub line: u32,
    pub doc: Option<String>,
    /// Where the `@moduledoc` sits, for styling in the code pane.
    pub doc_range: Option<Range>,
    pub functions: Vec<FnInfo>,
    /// What this module reaches outside itself.
    pub deps: Vec<Dep>,
}

/// What shape a file came in.
///
/// A module is not the only thing an Elixir file can be, and the blocks that
/// make sense for one make no sense for another: a config has no functions to
/// size, a test suite has no public surface. Anything unrecognised is `Plain`
/// and gets a blank page rather than four empty blocks, which read as broken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Module,
    Config,
    Test,
    Plain,
}

/// Where a configured value comes from — the one thing worth seeing in a config
/// file, because it is the difference between a value you can change at deploy
/// time and one baked into the release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ValueSource {
    /// Written in the file.
    Literal { value: String },
    /// `System.get_env/1` or `System.fetch_env!/1`; `required` marks the bang.
    Env { var: String, required: bool },
    /// A literal that looks like a credential. The value is deliberately not
    /// carried — the doc gets pasted into PR comments — but the fact that it is
    /// hardcoded at all is the finding.
    Secret,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Setting {
    pub key: String,
    pub line: u32,
    /// Last line of the block, so selecting it covers the whole thing
    /// rather than just its opening line.
    pub end_line: u32,
    pub source: ValueSource,
}

/// One `config :app, Target, …` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigGroup {
    /// `:my_app`
    pub app: String,
    /// `MyApp.Repo` or `:console` — absent for the two-arity form.
    pub target: Option<String>,
    pub line: u32,
    /// Last line of the block, so selecting it covers the whole thing
    /// rather than just its opening line.
    pub end_line: u32,
    pub settings: Vec<Setting>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigInfo {
    pub groups: Vec<ConfigGroup>,
    /// `import_config "dev.secret.exs"` — the load chain, later wins.
    pub imports: Vec<String>,
}

/// A `setup` or `setup_all` block.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupInfo {
    /// `setup` or `setup_all`.
    pub kind: String,
    pub line: u32,
    /// Last line of the block, so selecting it covers the whole thing
    /// rather than just its opening line.
    pub end_line: u32,
    /// `setup :put_user` — a callback defined elsewhere in the file.
    pub named: Option<String>,
    /// Context keys the block returns, read from its last expression.
    /// `None` means unknown, which is different from "provides nothing".
    pub provides: Option<Vec<String>>,
}

/// What an assertion checks.
///
/// Three kinds, because three is what the grammar can tell apart **without
/// guessing**, and each answers a different question about a suite:
///
///   · `Assert`  — the good path
///   · `Error`   — failure is checked at all (`refute`, `assert_raise`)
///   · `Message` — something is waited on (`assert_receive`)
///
/// A count cannot say any of that. `count_asserts` used to return a `u32` on the
/// grounds that "a one-assertion test looks different from a five-assertion one",
/// and it does — but what it looks different *about* is the author's style, not
/// the test's thoroughness: one `assert {:ok, %User{email: ^e, id: id}} = …`
/// checks four things and counted as one. The kind is a fact; the count was a
/// proxy being read as a finding.
///
/// `assert {:error, cs} = …` is deliberately **not** classified as `Error`. The
/// pattern is in the arguments, not the call name, so recognising it means
/// matching text — and the house rule is that an uncertain answer is drawn plain
/// rather than confidently wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AssertKind {
    Assert,
    Error,
    Message,
}

/// One assertion, where it is and what it checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Assertion {
    pub line: u32,
    pub kind: AssertKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestCase {
    pub name: String,
    pub line: u32,
    /// Last line of the block, so selecting it covers the whole thing
    /// rather than just its opening line.
    pub end_line: u32,
    /// How many assertions — kept as `assertions.len()`, so nothing that reads
    /// the count breaks while the count-shading it fed is retired.
    pub asserts: u32,
    /// Every assertion, in source order, with its line and its kind.
    pub assertions: Vec<Assertion>,
    /// The `@tag`s above the test, as their own span.
    ///
    /// Not folded into `line`/`end_line`: a tag is to a test what an `@spec` is
    /// to a function — the contract, not the body — so it is carried separately
    /// and drawn in `--mark`, exactly as `spec_range` is. Folding it in would
    /// say the tag is part of the test's code; leaving it out entirely means
    /// selecting a skipped test dims `@tag :skip` one line above it, which is
    /// the single most important fact about that test.
    pub tag_range: Option<Range>,
    pub tags: Vec<String>,
    pub skipped: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Describe {
    /// `None` for tests written directly in the module body.
    pub name: Option<String>,
    pub line: u32,
    /// Last line of the block, so selecting it covers the whole thing
    /// rather than just its opening line.
    pub end_line: u32,
    pub setups: Vec<SetupInfo>,
    pub tests: Vec<TestCase>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestInfo {
    pub module: String,
    /// `use MyApp.DataCase` — what the suite is built on.
    pub case_template: Option<String>,
    pub is_async: bool,
    /// Module-scope setups: every test in the file inherits these.
    pub setups: Vec<SetupInfo>,
    /// `@moduletag :skip` — applies to every test in the file.
    ///
    /// Kept apart from a test's own tags because it is a different fact. They
    /// used to be the same one: `attribute_tag` matched `moduletag` too, so a
    /// `@moduletag` at the top of a file was pushed onto the pending list and
    /// silently became the *first test's* tag.
    pub module_tags: Vec<String>,
    pub describes: Vec<Describe>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Outline {
    pub lang: String,
    pub kind: FileKind,
    pub modules: Vec<ModuleInfo>,
    pub config: Option<ConfigInfo>,
    pub tests: Option<TestInfo>,
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
