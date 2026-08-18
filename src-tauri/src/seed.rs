//! Turning an [`Outline`] into the starter markdown doc.
//!
//! The seed is deliberately mostly-empty: it lays out every function with a
//! blank explanation so the gaps are visible. Those gaps are the nudge — an
//! unexplained function renders as a ghost "explain…" placeholder.

use crate::db::models::FileHistory;
use crate::parse::{ModuleInfo, Outline, Visibility};

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

/// Build the whole starter doc.
///
/// Every block is written out with its **values already in it**. Nothing is
/// computed at render time: the markdown file is the data, so it stays
/// readable as plain text, survives being copied anywhere, and can be edited
/// by hand when you disagree with it.
pub fn seed_markdown(outline: &Outline, source: &str, history: &FileHistory) -> String {
    let Some(module) = outline.modules.first() else {
        return "# Untitled\n\n> _what is this file for?_\n".to_string();
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

    // How big is this, what is in it, what shape is it, what does it touch —
    // and only then the block you write in. The directory comes before the
    // pictures: names are what you orient by, and the treemap and reach diagram
    // both read better once you already know what the names are.
    out.push_str(&stats_block(module, source, history));
    out.push('\n');

    out.push_str("## Surface\n\n");
    out.push_str(&surface_block(module));
    out.push('\n');

    out.push_str("## Shape\n\n");
    out.push_str(&treemap_block(module));
    out.push('\n');

    if !module.deps.is_empty() {
        out.push_str("## Reach\n\n");
        out.push_str(&deps_block(module));
        out.push('\n');
    }

    // The functions block comes last: everything above it is generated, and
    // this is the one you write in.
    out.push_str("## Explain\n\n");
    out.push_str(&functions_block(module));
    out.push_str("\n## Notes\n\n");
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

/// The ```lgtm:functions block for one module.
pub fn functions_block(module: &ModuleInfo) -> String {
    let mut out = format!("```{BLOCK_TAG} module={}\n", module.name);

    for visibility in [Visibility::Public, Visibility::Private] {
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

    /// Just the ```lgtm:functions block, so assertions about the table aren't
    /// confused by the treemap listing the same names.
    fn functions_block_of(md: &str) -> String {
        md.split("```lgtm:functions")
            .nth(1)
            .and_then(|b| b.split("```").next())
            .expect("functions block")
            .to_string()
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
        )
    }

    #[test]
    fn titles_from_the_module_and_quotes_the_moduledoc() {
        let md = seeded();
        assert!(md.starts_with("# MyApp.Accounts\n"));
        assert!(md.contains("> Reads and writes for the users table."));
    }

    #[test]
    fn groups_public_and_private() {
        let md = seeded();
        let pub_at = md.find("public:").expect("public group");
        let priv_at = md.find("private:").expect("private group");
        assert!(pub_at < priv_at, "public comes first");
        assert!(md.contains("normalize/1"));
    }

    #[test]
    fn carries_existing_docs_as_prose_and_leaves_the_rest_blank() {
        let md = seeded();
        assert!(md.contains("create_user/1"));
        assert!(md.contains("Creates a user."));
        // get_user!/1 has no @doc, so its explanation is an empty slot.
        // Scope to the functions block — the treemap also lists every name.
        let table = functions_block_of(&md);
        let line = table
            .lines()
            .find(|l| l.contains("get_user!/1"))
            .expect("row present");
        assert!(line.trim_end().ends_with(':'), "empty slot, got: {line}");
    }

    #[test]
    fn renders_default_arguments_as_a_range() {
        assert!(seeded().contains("search/1..2"));
    }

    #[test]
    fn is_a_well_formed_fence() {
        let md = seeded();
        assert!(md.contains("```lgtm:functions module=MyApp.Accounts"));
        // stats, treemap, surface, functions — the sample reaches nothing
        // outside itself, so deps is absent and four blocks remain.
        assert_eq!(md.matches("```").count(), 8, "every block opened and closed");
    }

    #[test]
    fn seeds_the_treemap_alongside_the_table() {
        let md = seeded();
        assert!(md.contains("```lgtm:treemap"));
        // Its body carries the sizes as text — the markdown IS the data.
        assert!(md.contains("```lgtm:treemap\n  "), "{md}");
        // Facts, then the directory, then the pictures, then where you write.
        assert!(md.find("lgtm:stats").unwrap() < md.find("lgtm:surface").unwrap());
        assert!(md.find("lgtm:surface").unwrap() < md.find("lgtm:treemap").unwrap());
        assert!(md.find("lgtm:treemap").unwrap() < md.find("lgtm:functions").unwrap());
    }

    #[test]
    fn the_surface_block_is_a_directory_sorted_by_name() {
        let md = seeded();
        let block = md
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
        let md = seeded();
        let table = functions_block_of(&md);
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
        let md = seed_markdown(&elixir::parse(src).unwrap(), src, &FileHistory::default());
        let block = md
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
        let md = seed_markdown(&elixir::parse(src).unwrap(), src, &FileHistory::default());
        assert!(!md.contains("lgtm:deps"), "nothing to show, so no block:\n{md}");
        assert!(!md.contains("## Reach"));
    }

    #[test]
    fn a_file_with_no_module_still_seeds_something() {
        let outline = elixir::parse("x = 1\n").unwrap();
        assert!(seed_markdown(&outline, "x = 1\n", &FileHistory::default()).starts_with("# Untitled"));
    }

    #[test]
    fn the_stats_block_carries_its_numbers_as_text() {
        let md = seeded();
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

    #[test]
    fn the_treemap_block_carries_its_sizes_as_text() {
        let md = seeded();
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
        let md = seed_markdown(&elixir::parse(SAMPLE).unwrap(), SAMPLE, &FileHistory::default());
        assert!(md.contains("lines: "), "size still reported");
        assert!(!md.contains("commits:"), "no git, no git columns");
        assert!(!md.contains("authors:"));
    }
}
