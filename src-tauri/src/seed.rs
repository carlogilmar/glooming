//! Turning an [`Outline`] into the starter markdown doc.
//!
//! The seed is deliberately mostly-empty: it lays out every function with a
//! blank explanation so the gaps are visible. Those gaps are the nudge — an
//! unexplained function renders as a ghost "explain…" placeholder.

use crate::db::models::FileHistory;
use crate::parse::{
    ConfigInfo, FileKind, ModuleInfo, Outline, SetupInfo, TestInfo, ValueSource, Visibility,
};

/// The fence tag the frontend renderer looks for.
pub const BLOCK_TAG: &str = "lgtm:functions";
/// The function-size treemap. Its body is empty on purpose — the renderer
/// draws from the live outline, since the shape of the code is not something
/// you hand-edit.
pub const TREEMAP_TAG: &str = "lgtm:treemap";
/// File-level facts: size, surface, and what git knows. Body empty for the
/// same reason as the treemap — none of it is hand-written.
pub const STATS_TAG: &str = "lgtm:stats";
/// What the module reaches outside itself. Same rule again: the block carries
/// its own data, so the picture is drawn from the text.
pub const DEPS_TAG: &str = "lgtm:deps";
/// The module's surface as a directory: public and private, sorted by name.
/// Distinct from `lgtm:functions`, which is where *you* write.
pub const SURFACE_TAG: &str = "lgtm:surface";
/// A config script's settings, grouped by app.
pub const SETTINGS_TAG: &str = "lgtm:settings";
/// A test suite's describes, setups and tests.
pub const TESTS_TAG: &str = "lgtm:tests";

/// Build the whole starter doc.
///
/// Every block is written out with its **values already in it**. Nothing is
/// computed at render time: the markdown file is the data, so it stays
/// readable as plain text, survives being copied anywhere, and can be edited
/// by hand when you disagree with it.
pub fn seed_markdown(
    outline: &Outline,
    source: &str,
    history: &FileHistory,
    filename: &str,
) -> String {
    match outline.kind {
        FileKind::Config => return seed_config(outline, source, history, filename),
        FileKind::Test => return seed_test(outline, source, history),
        FileKind::Plain => return seed_plain(source, history, filename),
        FileKind::Module => {}
    }

    let Some(module) = outline.modules.first() else {
        return seed_plain(source, history, filename);
    };
    let mut out = format!("# {}\n\n", module.name);

    match &module.doc {
        Some(doc) if !doc.is_empty() => {
            for line in doc.lines() {
                out.push_str(&format!("> {line}\n"));
            }
            out.push('\n');
        }
        _ => out.push_str("> _one-line summary…_\n\n"),
    }

    // **A module doc is a title, a summary and a blank page.** Nothing is
    // generated into it.
    //
    // Every block that used to be here now lives in the explore drawer beside the
    // note, for whichever file is open: `lgtm:surface` and `lgtm:deps` are what
    // you *navigate* by, and navigation belongs next to the code, not pinned to a
    // position in a narrative. That is what made a multi-file reading uneven —
    // only the file the reading started from had them.
    //
    // `lgtm:stats` went for a different reason: size and history are not
    // consulted while navigating, they are context you want *recorded*. `/stats`
    // puts them where your prose wants them, and they travel with the text.
    //
    // All five blocks stay renderable and reconcilable; `/` inserts any of them
    // when your explanation deliberately wants one. See `block_for`.
    out.push_str("## Explain\n\n");
    out.push_str(NOTES_INVITE);
    out
}

/// What the Explain section says on a doc nobody has written in yet.
///
/// It is now the *only* thing a module doc is seeded with, besides the title and
/// the moduledoc — so it carries more weight than it did, and names the two keys
/// that turn an empty note into a reading.
///
/// It used to be a generated list of every private helper, each an inline
/// reference — which existed so `▷ Read` did something on a file you had not
/// written a word about. `/` does that job now, and better: you reference the
/// functions your explanation actually reaches, in the order your prose takes
/// them, rather than scrolling a machine-made list of every helper in source
/// order. A list of things you did not choose was never really a reading.
///
/// So this is an invitation and nothing more. One italic paragraph, and it names
/// the two keys that turn an empty note into a reading — because with nothing
/// generated here, `▷ Read` stays hidden until the first reference exists, and
/// that is worth saying rather than leaving to be discovered.
const NOTES_INVITE: &str = "_The code is on the left — write what you make of it here. \
     Press `/` while editing to reference any function in this reading; once your prose \
     names one, `▷ Read` will walk the code in the order you wrote it._\n";

/// A config script. Like every other kind, it is seeded with a title and a blank
/// page: `lgtm:settings` is what you *navigate* a config by, and navigation lives
/// in the explore drawer beside the code.
fn seed_config(
    _outline: &Outline,
    _source: &str,
    _history: &FileHistory,
    filename: &str,
) -> String {
    let title = if filename.is_empty() { "Configuration" } else { filename };
    let mut out = format!("# {title}\n\n> _what does this file configure?_\n\n");
    out.push_str("## Explain\n\n");
    out.push_str(PLAIN_INVITE);
    out
}

/// A test suite. Its structure — describes, setups, assertion counts — is in the
/// drawer, which is where you read it from. Nothing about it is seeded.
fn seed_test(outline: &Outline, _source: &str, _history: &FileHistory) -> String {
    let tests = outline.tests.clone().unwrap_or_default();
    let mut out = format!("# {}\n\n> _what does this suite cover?_\n\n", tests.module);
    out.push_str("## Explain\n\n");
    out.push_str(PLAIN_INVITE);
    out
}

/// Anything else — a script, a one-off `.exs`, a file that didn't parse. A
/// title, the size, and a blank page. No blocks, and above all no error.
fn seed_plain(_source: &str, _history: &FileHistory, filename: &str) -> String {
    let title = if filename.is_empty() { "Untitled" } else { filename };
    let mut out = format!("# {title}\n\n> _what is this file for?_\n\n");
    // Say *why* there is nothing, so the absence reads as a fact about the file
    // rather than as something broken. This is the whole point of the `Plain`
    // kind, and the one sentence still worth seeding.
    out.push_str(
        "_No module, config or test suite recognised in this file — so there is nothing to \
         navigate by, and the drawer stays quiet. The code is on the left._\n",
    );
    out.push_str("\n## Explain\n\n");
    out.push_str(PLAIN_INVITE);
    out
}

/// The same invitation for the kinds that have no functions to reference, so `/`
/// and `▷ Read` are not promised where they do nothing.
const PLAIN_INVITE: &str = "_The code is on the left — write what you make of it here._\n";

/// The shared tail of every stats block: the git facts, when there are any.
fn stats_lines(rows: &[(&str, String)], history: &FileHistory) -> String {
    let mut out = format!("```{STATS_TAG}\n");
    for (k, v) in rows {
        out.push_str(&format!("{k}: {v}\n"));
    }
    if history.commits > 0 {
        out.push_str(&format!("commits: {}\n", history.commits));
        if !history.authors.is_empty() {
            out.push_str(&format!("authors: {}\n", history.authors.join(", ")));
        }
        if let Some(first) = &history.first {
            out.push_str(&format!("created: {}\n", date_only(first)));
        }
        if let Some(last) = &history.last {
            out.push_str(&format!("updated: {}\n", date_only(last)));
        }
    }
    out.push_str("```\n");
    out
}

pub fn config_stats_block(config: &ConfigInfo, source: &str, history: &FileHistory) -> String {
    let all: Vec<_> = config.groups.iter().flat_map(|g| &g.settings).collect();
    let env = all
        .iter()
        .filter(|s| matches!(s.source, ValueSource::Env { .. }))
        .count();
    let required = all
        .iter()
        .filter(|s| matches!(s.source, ValueSource::Env { required: true, .. }))
        .count();
    let masked = all
        .iter()
        .filter(|s| matches!(s.source, ValueSource::Secret))
        .count();
    let apps: std::collections::BTreeSet<_> = config.groups.iter().map(|g| &g.app).collect();

    let mut rows = vec![
        ("lines", source.lines().count().to_string()),
        ("apps", apps.len().to_string()),
        ("groups", config.groups.len().to_string()),
        ("settings", all.len().to_string()),
        ("fromEnv", format!("{env} ({required} required)")),
        ("literal", format!("{} ({masked} masked)", all.len() - env)),
    ];
    if !config.imports.is_empty() {
        rows.push(("imports", config.imports.join(", ")));
    }
    stats_lines(&rows, history)
}

pub fn test_stats_block(tests: &TestInfo, source: &str, history: &FileHistory) -> String {
    let all: Vec<_> = tests.describes.iter().flat_map(|d| &d.tests).collect();
    let asserts: u32 = all.iter().map(|t| t.asserts).sum();
    let named = tests.describes.iter().filter(|d| d.name.is_some()).count();
    let setups = tests.setups.len() + tests.describes.iter().map(|d| d.setups.len()).sum::<usize>();
    let tags: std::collections::BTreeSet<_> =
        all.iter().flat_map(|t| t.tags.iter()).cloned().collect();

    let per = if all.is_empty() {
        "0".to_string()
    } else {
        format!("{:.1}", f64::from(asserts) / all.len() as f64)
    };

    let mut rows = vec![
        ("lines", source.lines().count().to_string()),
        ("tests", all.len().to_string()),
        ("describes", named.to_string()),
        ("assertions", format!("{asserts} ({per} per test)")),
        ("setups", format!("{setups} ({} module-wide)", tests.setups.len())),
        ("async", tests.is_async.to_string()),
    ];
    if let Some(c) = &tests.case_template {
        rows.push(("case", c.clone()));
    }
    if !tags.is_empty() {
        rows.push((
            "tagged",
            tags.iter().map(|t| format!("@{t}")).collect::<Vec<_>>().join(", "),
        ));
    }
    stats_lines(&rows, history)
}

/// The ```lgtm:settings block. Two levels by indent, like `lgtm:deps`: the
/// group, then its keys with where each value comes from.
pub fn settings_block(config: &ConfigInfo, filename: &str) -> String {
    let mut out = format!("```{SETTINGS_TAG} file={filename}\n");

    for g in &config.groups {
        let target = g.target.as_deref().map(|t| format!(" {t}")).unwrap_or_default();
        out.push_str(&format!("  {}{} : {}\n", g.app, target, span(g.line, g.end_line)));

        let width = g.settings.iter().map(|s| s.key.len()).max().unwrap_or(0);
        for s in &g.settings {
            let key = &s.key;
            let value = match &s.source {
                ValueSource::Env { var, required } => {
                    format!("env{} {var}", if *required { "!" } else { "" })
                }
                ValueSource::Secret => "secret".to_string(),
                ValueSource::Literal { value } => format!("= {value}"),
            };
            out.push_str(&format!(
                "    {key:width$} : {} {value}\n",
                span(s.line, s.end_line)
            ));
        }
    }

    for i in &config.imports {
        out.push_str(&format!("  import_config : {i}\n"));
    }
    out.push_str("```\n");
    out
}

/// `12` when a block is one line, `12-40` when it spans several — so selecting
/// a row can highlight the whole thing rather than just where it starts.
fn span(start: u32, end: u32) -> String {
    if end > start {
        format!("{start}-{end}")
    } else {
        start.to_string()
    }
}

fn setup_row(s: &SetupInfo, indent: &str) -> String {
    let what = match (&s.named, &s.provides) {
        (Some(n), _) => format!("runs :{n}"),
        (None, Some(keys)) if !keys.is_empty() => {
            keys.iter().map(|k| format!(":{k}")).collect::<Vec<_>>().join(" ")
        }
        (None, Some(_)) => "-".to_string(),
        // Unknown, which is not the same as "provides nothing".
        (None, None) => "?".to_string(),
    };
    format!("{indent}{} : {} {what}\n", s.kind, span(s.line, s.end_line))
}

/// The ```lgtm:tests block: module setups, then each describe with its own
/// setups and tests.
pub fn tests_block(tests: &TestInfo) -> String {
    let mut out = format!("```{TESTS_TAG} module={}\n", tests.module);

    for s in &tests.setups {
        out.push_str(&setup_row(s, "  "));
    }

    for d in &tests.describes {
        match &d.name {
            Some(name) => {
                out.push_str(&format!("  describe \"{name}\" : {}\n", span(d.line, d.end_line)))
            }
            None => out.push_str(&format!("  (no describe) : {}\n", span(d.line, d.end_line))),
        }
        for s in &d.setups {
            out.push_str(&setup_row(s, "    "));
        }

        let width = d.tests.iter().map(|t| t.name.len()).max().unwrap_or(0);
        for t in &d.tests {
            let name = &t.name;
            let tags: String = t.tags.iter().map(|g| format!(" @{g}")).collect();
            out.push_str(&format!(
                "    {name:width$} : {} {}{tags}\n",
                span(t.line, t.end_line),
                t.asserts
            ));
        }
    }

    out.push_str("```\n");
    out
}

/// The ```lgtm:stats block: file-level facts as `key: value` lines.
pub fn stats_block(module: &ModuleInfo, source: &str, history: &FileHistory) -> String {
    let lines = source.lines().count();
    let code = source.lines().filter(|l| !l.trim().is_empty()).count();
    let publics = module
        .functions
        .iter()
        .filter(|f| f.visibility == Visibility::Public)
        .count();
    let privates = module.functions.len() - publics;

    let mut out = format!("```{STATS_TAG}\n");
    out.push_str(&format!("lines: {lines}\n"));
    out.push_str(&format!("code: {code}\n"));
    out.push_str(&format!("public: {publics}\n"));
    out.push_str(&format!("private: {privates}\n"));

    // Git columns only when there is git to speak of.
    if history.commits > 0 {
        out.push_str(&format!("commits: {}\n", history.commits));
        if !history.authors.is_empty() {
            out.push_str(&format!("authors: {}\n", history.authors.join(", ")));
        }
        if let Some(first) = &history.first {
            out.push_str(&format!("created: {}\n", date_only(first)));
        }
        if let Some(last) = &history.last {
            out.push_str(&format!("updated: {}\n", date_only(last)));
        }
    }
    out.push_str("```\n");
    out
}

/// ISO-8601 timestamp to a bare date — the time of day is noise here.
fn date_only(iso: &str) -> &str {
    iso.split('T').next().unwrap_or(iso)
}

/// The ```lgtm:treemap block: one `sig : lines visibility` row per function,
/// mirroring Alexandria's `label: value flags` treemap syntax.
///
/// **No longer part of seeding** — see `seed_markdown`. Kept because it is the
/// canonical way to write one, and because a block whose only producer is a
/// hand-typed guess drifts away from its renderer. Pinned by
/// `the_treemap_is_still_a_block_even_though_it_is_not_seeded`.
pub fn treemap_block(module: &ModuleInfo) -> String {
    let mut out = format!("```{TREEMAP_TAG}\n");

    let mut fns: Vec<_> = module.functions.iter().collect();
    // Biggest first: the rows read as a ranking even before they're a chart.
    fns.sort_by(|a, b| lines_of(b).cmp(&lines_of(a)).then(a.name.cmp(&b.name)));

    let width = fns.iter().map(|f| display_sig(f).len()).max().unwrap_or(0);
    for f in fns {
        let sig = display_sig(f);
        let vis = match f.visibility {
            Visibility::Public => "public",
            Visibility::Private => "private",
        };
        out.push_str(&format!("  {sig:width$} : {} {vis}\n", lines_of(f)));
    }
    out.push_str("```\n");
    out
}

/// Lines a function occupies, summed across every clause.
fn lines_of(f: &crate::parse::FnInfo) -> u32 {
    if f.clause_ranges.is_empty() {
        return f.end_line.saturating_sub(f.line) + 1;
    }
    f.clause_ranges
        .iter()
        .map(|r| r.end.saturating_sub(r.start) + 1)
        .sum()
}

/// The ```lgtm:deps block.
///
/// Two levels, by indent: a module line, then the functions of it this file
/// actually calls, each followed by the local functions doing the calling.
/// Everything after the first colon is the value, as in every other block.
pub fn deps_block(module: &ModuleInfo) -> String {
    let mut out = format!("```{DEPS_TAG} module={}\n", module.name);

    for dep in &module.deps {
        out.push_str(&format!("  {} : {}\n", dep.module, dep.kind.as_str()));

        let width = dep
            .functions
            .iter()
            .map(|f| f.name.len())
            .max()
            .unwrap_or(0);
        for f in &dep.functions {
            let name = &f.name;
            out.push_str(&format!("    {name:width$} : {}\n", f.callers.join(", ")));
        }
    }

    out.push_str("```\n");
    out
}

/// The ```lgtm:surface block: every function, sorted by name, split by
/// visibility. A directory to look things up in — which is why it is sorted by
/// name and not by position, and why it carries the line number as the only
/// remaining hint of where you are going.
pub fn surface_block(module: &ModuleInfo) -> String {
    let mut out = format!("```{SURFACE_TAG} module={}\n", module.name);

    for visibility in [Visibility::Public, Visibility::Private] {
        let mut group: Vec<_> = module
            .functions
            .iter()
            .filter(|f| f.visibility == visibility)
            .collect();
        group.sort_by(|a, b| a.name.cmp(&b.name).then(a.arity.cmp(&b.arity)));
        if group.is_empty() {
            continue;
        }

        out.push_str(match visibility {
            Visibility::Public => "public:\n",
            Visibility::Private => "private:\n",
        });

        let width = group.iter().map(|f| display_sig(f).len()).max().unwrap_or(0);
        for f in &group {
            let sig = display_sig(f);
            let mut flags = String::new();
            if f.min_arity < f.arity {
                flags.push_str(" default args");
            }
            if f.clauses > 1 {
                flags.push_str(&format!(" {} clauses", f.clauses));
            }
            out.push_str(&format!("  {sig:width$} : {}{flags}\n", f.line));
        }
    }

    out.push_str("```\n");
    out
}

/// The ```lgtm:functions block: **the public surface only**.
///
/// **No longer part of seeding** — see `seed_markdown`. Kept, tested, rendered and
/// reconciled, because it is still the right shape when you *want* a slot per
/// function; it is just not what a doc should open with. Private helpers are
/// deliberately absent: the block carries what the module *offers*, and they are
/// one `/` away when your explanation needs to reach one.
pub fn functions_block(module: &ModuleInfo) -> String {
    let mut out = format!("```{BLOCK_TAG} module={}\n", module.name);

    for visibility in [Visibility::Public] {
        // Alphabetical, not source order: the table is a directory you look
        // things up in, and source order is already the treemap's job.
        let mut group: Vec<_> = module
            .functions
            .iter()
            .filter(|f| f.visibility == visibility)
            .collect();
        group.sort_by(|a, b| a.name.cmp(&b.name).then(a.arity.cmp(&b.arity)));
        if group.is_empty() {
            continue;
        }
        out.push_str(match visibility {
            Visibility::Public => "public:\n",
            Visibility::Private => "private:\n",
        });

        // Pad signatures so the colons line up in the raw source — the doc is
        // read as text as often as it is rendered.
        let width = group.iter().map(|f| display_sig(f).len()).max().unwrap_or(0);
        for f in &group {
            let sig = display_sig(f);
            // An existing @doc becomes the starting prose; otherwise blank.
            let prose = f.doc.as_deref().unwrap_or("").replace('\n', " ");
            out.push_str(&format!("  - {sig:width$} : {prose}\n"));
        }
    }

    out.push_str("```\n");
    out
}

/// `create_user/1`, or `search/1..2` when the definition has default arguments.
fn display_sig(f: &crate::parse::FnInfo) -> String {
    if f.min_arity < f.arity {
        format!("{}/{}..{}", f.name, f.min_arity, f.arity)
    } else {
        format!("{}/{}", f.name, f.arity)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::elixir;

    const SAMPLE: &str = r#"defmodule MyApp.Accounts do
  @moduledoc "Reads and writes for the users table."

  @doc "Creates a user."
  def create_user(attrs), do: attrs

  def get_user!(id), do: id

  def search(term, opts \\ []), do: {term, opts}

  defp normalize(attrs), do: attrs
end
"#;

    /// The ```lgtm:functions block on its own. Seeding no longer writes one, so
    /// this takes the generator's output directly — several blocks list every
    /// function name, and an assertion that isn't scoped finds the wrong row.
    fn functions_block_of(md: &str) -> String {
        md.split("```lgtm:functions")
            .nth(1)
            .and_then(|b| b.split("```").next())
            .expect("functions block")
            .to_string()
    }

    /// The sample's module, for the tests that are about a generator rather than
    /// about what gets seeded. Nothing but `lgtm:functions` is seeded any more —
    /// surface and deps moved to the explore drawer, stats to `/stats` — so every
    /// block test drives its generator directly.
    fn sample_module() -> crate::parse::ModuleInfo {
        elixir::parse(SAMPLE).expect("parse").modules.into_iter().next().expect("module")
    }

    fn history() -> FileHistory {
        FileHistory {
            commits: 3,
            authors: vec!["Carlo Padilla".into(), "Jane Rivera".into()],
            first: Some("2025-02-14T09:00:00+00:00".into()),
            last: Some("2026-08-10T09:00:00+00:00".into()),
        }
    }

    /// The generated table, for the tests that are about the table itself.
    fn table() -> String {
        let outline = elixir::parse(SAMPLE).expect("parse");
        functions_block_of(&functions_block(outline.modules.first().expect("module")))
    }

    fn seeded() -> String {
        seed_markdown(
            &elixir::parse(SAMPLE).unwrap(),
            SAMPLE,
            &FileHistory {
                commits: 3,
                authors: vec!["Carlo Padilla".into(), "Jane Rivera".into()],
                first: Some("2025-02-14T09:00:00+00:00".into()),
                last: Some("2026-08-10T09:00:00+00:00".into()),
            },
            "accounts.ex",
        )
    }

    #[test]
    fn titles_from_the_module_and_quotes_the_moduledoc() {
        let md = seeded();
        assert!(md.starts_with("# MyApp.Accounts\n"));
        assert!(md.contains("> Reads and writes for the users table."));
    }

    /// The surface block is the only listing now, and it still splits by
    /// visibility with public first.
    /// The surface block still splits by visibility, public first — it is just
    /// rendered in the drawer now rather than seeded into the note.
    #[test]
    fn groups_public_and_private() {
        let block = surface_block(&sample_module());
        let pub_at = block.find("public:").expect("public group");
        let priv_at = block.find("private:").expect("private group");
        assert!(pub_at < priv_at, "public comes first");
        assert!(block.contains("normalize/1"), "privates are listed:\n{block}");
    }

    #[test]
    fn carries_existing_docs_as_prose_and_leaves_the_rest_blank() {
        let table = table();
        assert!(table.contains("create_user/1"));
        assert!(table.contains("Creates a user."));
        // get_user!/1 has no @doc, so its explanation is an empty slot.
        let line = table
            .lines()
            .find(|l| l.contains("get_user!/1"))
            .expect("row present");
        assert!(line.trim_end().ends_with(':'), "empty slot, got: {line}");
    }

    #[test]
    fn renders_default_arguments_as_a_range() {
        assert!(surface_block(&sample_module()).contains("search/1..2"));
    }

    /// **A seeded module doc has no blocks at all.** A title, the moduledoc as a
    /// blockquote, and an invitation to write — everything else is either in the
    /// drawer or one `/` away.
    #[test]
    fn a_seeded_module_doc_carries_no_blocks() {
        let md = seeded();
        assert_eq!(md.matches("```").count(), 0, "no fences:\n{md}");
        for absent in ["lgtm:stats", "lgtm:surface", "lgtm:deps", "lgtm:treemap", "lgtm:functions"] {
            assert!(!md.contains(absent), "{absent} is not seeded:\n{md}");
        }
        // And every generator still writes one when asked.
        assert!(table().contains("create_user/1"));
        assert!(surface_block(&sample_module()).starts_with("```lgtm:surface"));
        assert!(stats_block(&sample_module(), SAMPLE, &history()).starts_with("```lgtm:stats"));
    }

    /// Facts, then the directory, then a blank page. Two blocks, and everything
    /// after them is yours.
    /// What is left: a title, the moduledoc, and where you write.
    #[test]
    fn a_module_doc_is_a_title_a_summary_and_a_blank_page() {
        let md = seeded();
        assert!(md.starts_with("# MyApp.Accounts\n"));
        assert!(md.contains("> Reads and writes for the users table."));
        assert!(md.contains("## Explain"));
        assert!(md.contains("write what you make of it here"));
        // Nothing between the summary and the invitation — the slice starts after
        // the summary text, not at it.
        const SUMMARY: &str = "users table.";
        let from = md.find(SUMMARY).unwrap() + SUMMARY.len();
        let between = &md[from..md.find("## Explain").unwrap()];
        assert!(between.trim().is_empty(), "nothing seeded between:\n{between:?}");
    }

    #[test]
    fn the_surface_block_is_a_directory_sorted_by_name() {
        let full = surface_block(&sample_module());
        let block = full
            .split("```lgtm:surface")
            .nth(1)
            .and_then(|b| b.split("```").next())
            .expect("surface block");

        let names: Vec<String> = block
            .lines()
            .filter(|l| l.contains(" : "))
            .map(|l| l.trim().split(" : ").next().unwrap().trim().to_string())
            .collect();

        // Both groups present, each alphabetical within itself.
        assert!(block.contains("public:") && block.contains("private:"), "{block}");
        assert!(names.contains(&"create_user/1".to_string()), "{names:?}");
        // Line numbers ride along — sorting by name throws away source order,
        // so the line is the only hint of where a row goes.
        assert!(block.contains(" : 5"), "line numbers present:\n{block}");
        // Flags earn a badge.
        assert!(block.contains("default args"), "{block}");
    }

    #[test]
    fn the_table_is_alphabetical_within_each_group() {
        let table = table();
        let rows: Vec<&str> = table
            .lines()
            .filter(|l| l.trim_start().starts_with("- "))
            .collect();
        // Public group of the sample: create_user, get_user!, search.
        let names: Vec<String> = rows
            .iter()
            .map(|l| l.trim_start().trim_start_matches("- ").split('/').next().unwrap().to_string())
            .collect();
        let publics = &names[..3];
        let mut sorted = publics.to_vec();
        sorted.sort();
        assert_eq!(publics, &sorted[..], "public group sorted: {names:?}");
    }

    #[test]
    fn the_deps_block_carries_the_edges_as_text() {
        let src = r#"defmodule MyApp.Accounts do
  alias MyApp.Repo
  alias MyApp.User
  alias Ecto.Changeset

  def create_user(attrs) do
    %User{}
    |> Repo.insert()
  end

  defp normalize(a), do: Changeset.cast(a, %{}, [])
end
"#;
        let module = elixir::parse(src).unwrap().modules.into_iter().next().unwrap();
        let full = deps_block(&module);
        let block = full
            .split("```lgtm:deps")
            .nth(1)
            .and_then(|b| b.split("```").next())
            .expect("deps block");

        // A module line, then the functions of it that are actually called,
        // each naming who calls it.
        assert!(block.contains("MyApp.Repo : app"), "{block}");
        assert!(block.contains("Ecto.Changeset : lib"), "{block}");
        assert!(block.contains("insert/1"), "{block}");
        assert!(block.contains("create_user/1"), "{block}");
        assert!(block.contains("cast/3"), "{block}");
        assert!(block.contains("normalize/1"), "{block}");
    }

    #[test]
    fn a_module_that_reaches_nothing_gets_no_reach_section() {
        let src = "defmodule MyApp.Pure do\n  def add(a, b), do: a + b\nend\n";
        let md = seed_markdown(&elixir::parse(src).unwrap(), src, &FileHistory::default(), "accounts.ex");
        assert!(!md.contains("lgtm:deps"), "nothing to show, so no block:\n{md}");
        assert!(!md.contains("## Reach"));
    }

    #[test]
    fn a_file_with_no_module_still_seeds_something() {
        let outline = elixir::parse("x = 1\n").unwrap();
        assert!(seed_markdown(&outline, "x = 1\n", &FileHistory::default(), "").starts_with("# Untitled"));
    }

    /// `/stats` writes this into the note on request. Not seeded — size and
    /// history are context you want recorded, not something you navigate by.
    #[test]
    fn the_stats_block_carries_its_numbers_as_text() {
        let md = stats_block(&sample_module(), SAMPLE, &history());
        assert!(md.contains("lines: 12"), "{md}");
        assert!(md.contains("code: 8"));
        assert!(md.contains("public: 3"));
        assert!(md.contains("private: 1"));
        // Git facts, dates trimmed to the day.
        assert!(md.contains("commits: 3"));
        assert!(md.contains("authors: Carlo Padilla, Jane Rivera"));
        assert!(md.contains("created: 2025-02-14"));
        assert!(md.contains("updated: 2026-08-10"));
        assert!(!md.contains("T09:00:00"), "timestamps trimmed");
    }

    /// The generator is still exercised even though seeding no longer calls it —
    /// a block whose only producer is a hand-typed guess drifts from its renderer.
    #[test]
    fn the_treemap_block_carries_its_sizes_as_text() {
        let outline = elixir::parse(SAMPLE).expect("parse");
        let module = outline.modules.first().expect("module");
        let md = treemap_block(module);
        let block = md
            .split("```lgtm:treemap\n")
            .nth(1)
            .and_then(|b| b.split("```").next())
            .expect("treemap block");

        // Every function, with its line count and visibility.
        assert!(block.contains("create_user/1"), "{block}");
        assert!(block.contains("public"));
        assert!(block.contains("private"));
        assert_eq!(block.lines().filter(|l| l.contains(" : ")).count(), 4);
    }

    #[test]
    fn a_repo_less_file_omits_the_git_facts() {
        let md = stats_block(&sample_module(), SAMPLE, &FileHistory::default());
        assert!(md.contains("lines: "), "size still reported");
        assert!(!md.contains("commits:"), "no git, no git columns");
        assert!(!md.contains("authors:"));
    }
}

#[cfg(test)]
mod kind_tests {
    use super::*;
    use crate::parse;

    fn seed(src: &str) -> String {
        seed_markdown(
            &parse::parse(src, "elixir").unwrap(),
            src,
            &FileHistory::default(),
            "sample.exs",
        )
    }

    fn block_of<'a>(md: &'a str, tag: &str) -> &'a str {
        md.split(&format!("```{tag}"))
            .nth(1)
            .and_then(|b| b.split("```").next())
            .unwrap_or_else(|| panic!("no {tag} block in:\n{md}"))
    }

    /// The generators, run on a source. **No kind seeds a block any more** — a
    /// config's settings and a suite's describes are what you navigate those files
    /// by, and navigation lives in the explore drawer. Every block test therefore
    /// drives its generator, and `/settings` and `/tests` reach the same code.
    fn outline_of(src: &str) -> Outline {
        parse::parse(src, "elixir").expect("parse")
    }

    fn settings_of(src: &str) -> String {
        let o = outline_of(src);
        settings_block(&o.config.expect("config"), "sample.exs")
    }

    fn tests_of(src: &str) -> String {
        tests_block(&outline_of(src).tests.expect("tests"))
    }

    fn config_stats_of(src: &str) -> String {
        let o = outline_of(src);
        config_stats_block(&o.config.expect("config"), src, &FileHistory::default())
    }

    fn test_stats_of(src: &str) -> String {
        let o = outline_of(src);
        test_stats_block(&o.tests.expect("tests"), src, &FileHistory::default())
    }

    const CONFIG: &str = r#"import Config

config :my_app, MyApp.Repo,
  username: "postgres",
  password: System.fetch_env!("DB_PASSWORD"),
  signing_salt: "abc123"

config :logger, :console, level: :debug

import_config "dev.secret.exs"
"#;

    /// A config doc is a title and a blank page, like every other kind. Its
    /// settings are in the drawer, which is where you read them from.
    #[test]
    fn a_config_doc_is_a_title_and_a_blank_page() {
        let md = seed(CONFIG);
        assert!(md.starts_with("# sample.exs"), "{md}");
        assert!(md.contains("what does this file configure?"), "{md}");
        assert!(md.contains("## Explain"), "{md}");
        assert_eq!(md.matches("```").count(), 0, "no blocks at all:\n{md}");

        // And the generator still writes one when `/settings` asks.
        let block = settings_of(CONFIG);
        assert!(block.starts_with("```lgtm:settings"), "{block}");
        assert!(block.contains("my_app"), "{block}");
    }

    #[test]
    fn the_settings_block_says_where_each_value_comes_from() {
        let full = settings_of(CONFIG);
        let block = block_of(&full, "lgtm:settings");
        assert!(block.contains(":my_app MyApp.Repo"), "{block}");
        assert!(block.contains("env! DB_PASSWORD"), "required env:\n{block}");
        assert!(block.contains(r#"= "postgres""#), "literal value:\n{block}");
        // A hardcoded credential is reported without its value.
        assert!(block.contains("signing_salt"), "{block}");
        assert!(block.contains("secret"), "{block}");
        assert!(!block.contains("abc123"), "value must not ride along:\n{block}");
        assert!(block.contains("import_config : dev.secret.exs"), "{block}");
    }

    #[test]
    fn config_stats_count_what_matters_for_a_config() {
        let md = config_stats_of(CONFIG);
        let stats = block_of(&md, "lgtm:stats");
        assert!(stats.contains("apps: 2"), "{stats}");
        assert!(stats.contains("settings: 4"), "{stats}");
        assert!(stats.contains("fromEnv: 1 (1 required)"), "{stats}");
        assert!(stats.contains("literal: 3 (1 masked)"), "{stats}");
    }

    const SUITE: &str = r#"defmodule MyApp.AccountsTest do
  use MyApp.DataCase, async: true

  setup do
    %{user: user_fixture()}
  end

  describe "create_user/1" do
    setup :put_user

    @tag :slow
    test "creates a user" do
      assert true
      refute false
    end
  end

  test "loose" do
    assert true
  end
end
"#;

    /// A suite doc is a title and a blank page too.
    #[test]
    fn a_test_doc_is_a_title_and_a_blank_page() {
        let md = seed(SUITE);
        assert!(md.starts_with("# MyApp.AccountsTest"), "{md}");
        assert!(md.contains("what does this suite cover?"), "{md}");
        assert!(md.contains("## Explain"), "{md}");
        assert_eq!(md.matches("```").count(), 0, "no blocks at all:\n{md}");

        let block = tests_of(SUITE);
        assert!(block.starts_with("```lgtm:tests module=MyApp.AccountsTest"), "{block}");
    }

    #[test]
    fn the_tests_block_nests_setups_where_they_apply() {
        let md = tests_of(SUITE);
        let block = block_of(&md, "lgtm:tests");

        // Module scope first, then the describe with its own setup indented.
        let module_setup = block.lines().position(|l| l.trim().starts_with("setup :")).unwrap();
        let describe = block.lines().position(|l| l.contains("describe")).unwrap();
        assert!(module_setup < describe, "module scope leads:\n{block}");

        assert!(block.contains(":user"), "provided keys:\n{block}");
        // A named callback is unknown, not empty.
        assert!(block.contains("runs :put_user"), "{block}");
        assert!(block.contains("@slow"), "{block}");
        assert!(block.contains("(no describe)"), "loose tests still listed:\n{block}");
    }

    #[test]
    fn test_stats_count_what_matters_for_a_suite() {
        let md = test_stats_of(SUITE);
        let stats = block_of(&md, "lgtm:stats");
        assert!(stats.contains("tests: 2"), "{stats}");
        assert!(stats.contains("describes: 1"), "{stats}");
        assert!(stats.contains("assertions: 3"), "{stats}");
        assert!(stats.contains("async: true"), "{stats}");
        assert!(stats.contains("case: MyApp.DataCase"), "{stats}");
    }

    #[test]
    fn an_unrecognised_file_gets_a_blank_page_not_an_error() {
        let md = seed("IO.puts(\"hi\")\nx = 1\n");
        // Titled by filename — "Untitled" only when there isn't one.
        assert!(md.starts_with("# sample.exs"), "{md}");
        // And it says *why* there is nothing, rather than just being bare. This is
        // the whole point of the `Plain` kind and the one sentence still seeded.
        assert!(md.contains("nothing to navigate by"), "{md}");
        assert!(md.contains("## Explain"), "{md}");
        assert_eq!(md.matches("```").count(), 0, "no blocks:\n{md}");
    }
}

#[cfg(test)]
mod span_tests {
    use super::*;
    use crate::parse;

    const SUITE: &str = r#"defmodule MyApp.AccountsTest do
  use MyApp.DataCase

  setup do
    %{user: user_fixture()}
  end

  describe "create_user/1" do
    test "creates a user" do
      assert true
      refute false
    end

    test "one liner", do: assert(true)
  end
end
"#;

    #[test]
    fn every_block_carries_its_whole_span() {
        let outline = parse::parse(SUITE, "elixir").unwrap();
        // The generator, not the seed — nothing is seeded for a test suite now.
        let md = tests_block(&outline.tests.expect("tests"));
        let block = md
            .split("```lgtm:tests")
            .nth(1)
            .and_then(|b| b.split("```").next())
            .unwrap();

        // setup do … end spans lines 4-6.
        assert!(block.contains("setup : 4-6"), "{block}");
        // The describe wraps everything from 8 to 15.
        assert!(block.contains("describe \"create_user/1\" : 8-15"), "{block}");
        // A multi-line test carries its body; selecting it covers the whole thing.
        assert!(block.contains("creates a user : 9-12"), "{block}");
        // A one-liner has nothing to span, so it stays a single number.
        // The `do:` one-liner form has no do_block, so its assertion is only
        // found by counting over the whole call.
        assert!(block.contains("one liner      : 14 1"), "{block}");
    }
}
