//! End-to-end check of the read → parse → seed → reconcile pipeline, run
//! against the exact file the UI mockup shows. If the app ever stops matching
//! `mockup/index.html`, this is where it shows up first.

use lgtm_lib::db::models::FileHistory;
use lgtm_lib::{parse, seed};

const FIXTURE: &str = include_str!("fixtures/accounts.ex");

fn outline() -> parse::Outline {
    parse::parse(FIXTURE, "elixir").expect("fixture parses")
}

#[test]
fn parses_the_mockups_module_exactly_as_drawn() {
    let o = outline();
    let m = &o.modules[0];

    assert_eq!(m.name, "MyApp.Accounts");

    let public: Vec<_> = m
        .functions
        .iter()
        .filter(|f| f.visibility == parse::Visibility::Public)
        .map(|f| f.signature())
        .collect();
    let private: Vec<_> = m
        .functions
        .iter()
        .filter(|f| f.visibility == parse::Visibility::Private)
        .map(|f| f.signature())
        .collect();

    // The mockup's header reads "3 public · 2 private".
    assert_eq!(public, ["create_user/1", "get_user/1", "get_user!/1"]);
    assert_eq!(private, ["normalize/1", "changeset/2"]);
}

#[test]
fn line_numbers_match_the_mockups_click_targets() {
    let o = outline();
    let at = |sig: &str| {
        o.modules[0]
            .functions
            .iter()
            .find(|f| f.signature() == sig)
            .unwrap_or_else(|| panic!("missing {sig}"))
            .line
    };

    // Exactly the data-line values wired into mockup/index.html.
    assert_eq!(at("create_user/1"), 12);
    assert_eq!(at("get_user/1"), 20);
    assert_eq!(at("get_user!/1"), 24);
    assert_eq!(at("normalize/1"), 30);
    assert_eq!(at("changeset/2"), 36);
}

#[test]
fn normalize_reports_both_clauses() {
    let o = outline();
    let n = o.modules[0]
        .functions
        .iter()
        .find(|f| f.signature() == "normalize/1")
        .unwrap();
    // Defined at lines 30 and 34 — one row, two clauses.
    assert_eq!(n.clauses, 2);
    assert_eq!(n.line, 30);
}

/// A fresh doc gives you the facts and then gets out of the way.
///
/// Two generated blocks — how big is this, what is in it — and a blank page. The
/// `lgtm:functions` table used to sit between them and your writing; it was a
/// second listing of the names `lgtm:surface` already gives you, and its one
/// unique offering, a prose slot per function, is the wrong shape for an
/// explanation. Explanations follow the path through a module. An alphabetical
/// index does not.
#[test]
fn the_seeded_doc_gives_you_the_facts_and_a_blank_page() {
    let md = seed::seed_markdown(&outline(), FIXTURE, &FileHistory::default(), "accounts.ex");

    assert!(md.starts_with("# MyApp.Accounts\n"));
    assert!(md.contains("Reads and writes for the `users` table."));

    // The directory is still complete: every function, both visibilities.
    let surface = md
        .split("```lgtm:surface")
        .nth(1)
        .and_then(|b| b.split("```").next())
        .expect("surface block");
    for sig in ["create_user/1", "get_user/1", "get_user!/1", "normalize/1", "changeset/2"] {
        assert!(surface.contains(sig), "{sig} missing from the directory:\n{surface}");
    }

    // And nothing is written for you.
    for absent in ["lgtm:functions", "lgtm:treemap"] {
        assert!(!md.contains(absent), "{absent} is not seeded:\n{md}");
    }
}

/// A freshly seeded doc invites you to write, and nothing more.
///
/// The Notes section used to carry a generated list of every private helper as
/// inline references, so `▷ Read` did something before you had written a word.
/// `/` replaced that: you reference what your explanation reaches, in the order
/// your prose takes it, instead of scrolling a machine-made list of every helper
/// in source order. A list of things you did not choose was never a reading.
#[test]
fn the_seed_invites_you_to_write_rather_than_writing_for_you() {
    let md = seed::seed_markdown(&outline(), FIXTURE, &FileHistory::default(), "accounts.ex");
    let notes = md.split("## Explain").nth(1).expect("explain section");

    assert!(
        !notes.contains("Private helpers"),
        "no generated list:\n{notes}"
    );
    // Not an empty heading either — an empty section reads as something missing.
    assert!(notes.contains("write what you make of it here"), "{notes}");
    // It names the two keys, because with nothing generated here `▷ Read` stays
    // hidden until the first reference exists.
    assert!(notes.contains("`/`") && notes.contains("▷ Read"), "{notes}");
    // And it does not invent references of its own.
    assert!(!notes.contains("normalize/1"), "{notes}");
    assert!(!notes.contains("changeset/2"), "{notes}");
}

/// The shape section is gone from seeding — but `lgtm:treemap` is still a block.
///
/// Function sizes answer a question you ask occasionally, so a generated section
/// above the part you write in was not paying for its space. The generator stays,
/// because a block whose only producer is a hand-typed guess drifts away from its
/// renderer.
#[test]
fn the_treemap_is_still_a_block_even_though_it_is_not_seeded() {
    let outline = outline();
    let md = seed::seed_markdown(&outline, FIXTURE, &FileHistory::default(), "accounts.ex");
    assert!(!md.contains("lgtm:treemap"), "not seeded:\n{md}");
    assert!(!md.contains("## Shape"), "and no orphan heading:\n{md}");

    let module = outline.modules.first().expect("module");
    let block = seed::treemap_block(module);
    assert!(block.starts_with("```lgtm:treemap"), "{block}");
    assert!(block.contains("create_user/1"), "still writes rows:\n{block}");
}

/// What a fresh module doc is made of, in order. Pinned because everything above
/// `## Explain` is generated, and every one of those sections is space taken from
/// the part you write in.
#[test]
fn a_seeded_module_doc_has_exactly_these_sections() {
    let md = seed::seed_markdown(&outline(), FIXTURE, &FileHistory::default(), "accounts.ex");
    let headings: Vec<&str> = md
        .lines()
        .filter(|l| l.starts_with("## "))
        .map(|l| l.trim_start_matches("## "))
        .collect();
    assert_eq!(headings, ["Surface", "Reach", "Explain"], "{md}");
}

/// A module with nothing private has no list to write — and now neither does one
/// with private helpers. Either way the tail must not be an orphan heading.
#[test]
fn the_explain_section_is_never_left_empty() {
    let src = "defmodule MyApp.Pure do\n  def add(a, b), do: a + b\nend\n";
    let md = seed::seed_markdown(
        &parse::parse(src, "elixir").unwrap(),
        src,
        &FileHistory::default(),
        "pure.ex",
    );
    assert!(!md.contains("Private helpers"), "{md}");
    assert!(!md.trim_end().ends_with("## Explain"), "not a bare heading:\n{md}");
    assert!(md.contains("write what you make of it here"), "{md}");
}

/// Reconciliation still works, and is still the reason `lgtm:functions` exists.
///
/// The block is no longer seeded, so the doc is built here the way you would build
/// one: take the generated table, fill in some slots, then change the file. That
/// is a better test than it was — it exercises the reconciler against a block,
/// rather than against whatever the seeder happened to emit.
#[test]
fn a_later_edit_to_the_file_never_costs_you_prose() {
    // You write explanations. Signatures are padded to the widest in their
    // group, so fill the slot by line rather than by exact string.
    fn explain(md: &str, sig: &str, prose: &str) -> String {
        md.lines()
            .map(|l| {
                if l.trim_start().starts_with(&format!("- {sig} ")) || l.trim_end() == format!("- {sig} :") {
                    format!("{}{prose}", l.trim_end())
                } else {
                    l.to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    let outline = outline();
    let module = outline.modules.first().expect("module");
    let doc = format!(
        "# MyApp.Accounts\n\n## Explain\n\n{}",
        seed::functions_block(module)
    );

    let written = explain(&doc, "create_user/1", " Entry point. Validates, then inserts.");
    // Both public — the block is the public surface.
    let written = explain(&written, "get_user/1", " The bang-free half of the pair.");
    assert!(written.contains("The bang-free half of the pair."), "setup:\n{written}");

    // …then the file changes: get_user!/1 is deleted, delete_user/1 appears.
    let edited = FIXTURE
        .replace(
            "  def get_user!(id) do\n    Repo.get!(User, id)\n  end\n",
            "  def delete_user(id) do\n    Repo.delete(User, id)\n  end\n",
        )
        .to_string();
    let after = parse::parse(&edited, "elixir").unwrap();
    let merged = lgtm_lib::reconcile::reconcile_markdown(&written, &after);

    // Your writing survives, whatever happened to the code.
    assert!(merged.contains("Entry point. Validates, then inserts."));
    assert!(merged.contains("The bang-free half of the pair."));
    // The new function shows up with an empty slot.
    assert!(merged.contains("delete_user/1"));
    // The deleted one is struck through, not erased.
    assert!(merged.contains("~~get_user!/1~~"), "{merged}");
}

/// A seeded doc has no `lgtm:functions`, so reconciling one is a no-op on the
/// prose. Worth pinning: it is the same guarantee a config doc has, and it means
/// the reconcile path cannot quietly rewrite a doc that has no table.
#[test]
fn reconciling_a_seeded_doc_leaves_it_alone() {
    let md = seed::seed_markdown(&outline(), FIXTURE, &FileHistory::default(), "accounts.ex");
    let merged = lgtm_lib::reconcile::reconcile_markdown(&md, &outline());
    assert_eq!(merged, md, "nothing to reconcile, nothing changed");
}

/// The frontend reads `endLine` and `minArity`. Rust's field names are snake
/// case, so the serde rename is load-bearing: without it the doc pane gets
/// `undefined` for the end line and highlights only the `def` line instead of
/// the whole function body.
#[test]
fn the_wire_format_is_camel_case() {
    let o = outline();
    let json = serde_json::to_string(&o.modules[0].functions[0]).expect("serializes");

    assert!(json.contains("\"endLine\""), "got: {json}");
    assert!(json.contains("\"minArity\""), "got: {json}");
    assert!(!json.contains("end_line"), "snake case leaked: {json}");
    assert!(!json.contains("min_arity"), "snake case leaked: {json}");
}

/// A focused function must cover its whole body, header through `end`.
#[test]
fn end_lines_span_the_whole_function_body() {
    let o = outline();
    let f = |sig: &str| {
        o.modules[0]
            .functions
            .iter()
            .find(|f| f.signature() == sig)
            .unwrap_or_else(|| panic!("missing {sig}"))
    };

    // create_user/1 runs from `def` on 12 to its `end` on 17.
    assert_eq!((f("create_user/1").line, f("create_user/1").end_line), (12, 17));
    assert_eq!((f("get_user/1").line, f("get_user/1").end_line), (20, 22));
    assert_eq!((f("get_user!/1").line, f("get_user!/1").end_line), (24, 26));
    assert_eq!((f("changeset/2").line, f("changeset/2").end_line), (36, 41));
}






/// Reconcile touches `lgtm:functions` and nothing else. A config or test doc
/// has no such block, so re-parsing must pass it through untouched rather than
/// mangling blocks it doesn't understand.
#[test]
fn reconciling_a_config_doc_changes_nothing() {
    let cfg = "import Config\n\nconfig :my_app, MyApp.Repo, pool_size: 10\n";
    let outline = parse::parse(cfg, "elixir").unwrap();
    let md = seed::seed_markdown(&outline, cfg, &FileHistory::default(), "dev.exs");

    assert!(md.contains("```lgtm:settings"));
    assert_eq!(
        lgtm_lib::reconcile::reconcile_markdown(&md, &outline),
        md,
        "a doc with no functions block is passed through verbatim"
    );
}
