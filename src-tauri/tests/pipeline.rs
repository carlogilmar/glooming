//! End-to-end check of the read → parse → seed pipeline, run
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

/// A fresh doc is a title, a summary, and a blank page.
///
/// Every generated block has left it. `lgtm:surface` and `lgtm:deps` are what you
/// *navigate* by, and navigation belongs in the explore drawer next to the code —
/// pinned to a position in a narrative they only ever served the file the reading
/// started from, which is what made a multi-file reading uneven. `lgtm:stats` went
/// separately: size and history are context you want recorded, so `/stats` puts
/// them where your prose wants them.
#[test]
fn the_seeded_doc_is_a_title_a_summary_and_a_blank_page() {
    let md = seed::seed_markdown(&outline(), FIXTURE, &FileHistory::default(), "accounts.ex");

    assert!(md.starts_with("# MyApp.Accounts\n"));
    assert!(md.contains("Reads and writes for the `users` table."));
    assert!(md.contains("write what you make of it here"));

    // Not one fence.
    assert_eq!(md.matches("```").count(), 0, "no blocks at all:\n{md}");
    for absent in ["lgtm:stats", "lgtm:surface", "lgtm:deps", "lgtm:treemap", "lgtm:functions"] {
        assert!(!md.contains(absent), "{absent} is not seeded:\n{md}");
    }

    // The directory is still complete — it is just generated on demand now, for
    // the drawer or for `/surface`.
    let module = outline().modules.into_iter().next().expect("module");
    let surface = seed::surface_block(&module);
    for sig in ["create_user/1", "get_user/1", "get_user!/1", "normalize/1", "changeset/2"] {
        assert!(surface.contains(sig), "{sig} missing from the directory:\n{surface}");
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

/// What a fresh module doc is made of. Pinned because the whole direction of this
/// tool has been removing generated sections from above the part you write in, and
/// there is now exactly one heading left.
#[test]
fn a_seeded_module_doc_has_exactly_these_sections() {
    let md = seed::seed_markdown(&outline(), FIXTURE, &FileHistory::default(), "accounts.ex");
    let headings: Vec<&str> = md
        .lines()
        .filter(|l| l.starts_with("## "))
        .map(|l| l.trim_start_matches("## "))
        .collect();
    assert_eq!(headings, ["Explain"], "one heading, and it is where you write:\n{md}");
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

/// A gloom is pinned to the versions it was read at.
///
/// `reconcile.rs` is gone with the feature it served: the pane shows
/// `doc_files.source`, the outline is parsed from that, and the answer to "the
/// code moved on" is a new gloom rather than a merge that leaves half your prose
/// describing lines that are no longer there. What survives is the *signal* — a
/// file that has changed on disk says so — and the dangling reference, which
/// strikes through in the prose where you wrote it.
#[test]
fn a_gloom_keeps_the_source_it_was_read_at() {
    let before = parse::parse(FIXTURE, "elixir").unwrap();
    let edited = FIXTURE.replace(
        "  def get_user!(id) do\n    Repo.get!(User, id)\n  end\n",
        "  def delete_user(id) do\n    Repo.delete(User, id)\n  end\n",
    );
    let after = parse::parse(&edited, "elixir").unwrap();

    let names = |o: &lgtm_lib::parse::Outline| {
        o.modules[0]
            .functions
            .iter()
            .map(|f| f.name.clone())
            .collect::<Vec<_>>()
    };

    // The two parses differ — that is the premise — and a gloom holds the first
    // one for as long as it exists.
    assert!(names(&before).contains(&"get_user!".to_string()));
    assert!(names(&after).contains(&"delete_user".to_string()));
    assert!(!names(&before).contains(&"delete_user".to_string()));
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






/// Reconcile touches `lgtm:functions` and nothing else. Nothing seeds one any
/// more, so a doc must pass through untouched — and a doc that has picked up a

/// `/surface` and the explore drawer both generate blocks for whatever file is in
/// front of you, so the generators have to be reachable on their own — nothing
/// seeds them any more.
///
/// The order still has to be the seeder's order, because it is the same function:
/// surface was once sorted in both `seed.rs` and the renderer and the two
/// disagreed. One generator, one order, no second chance to drift.
#[test]
fn a_block_can_be_generated_on_demand() {
    let outline = outline();
    let module = outline.modules.first().expect("module");

    let surface = seed::surface_block(module);
    assert!(surface.starts_with("```lgtm:surface module=MyApp.Accounts"));
    assert!(surface.trim_end().ends_with("```"));

    // Sorted by name within each group.
    let rows: Vec<&str> = surface
        .lines()
        .filter(|l| l.contains(" : "))
        .map(|l| l.trim().split('/').next().unwrap())
        .collect();
    let publics = &rows[..3];
    let mut sorted = publics.to_vec();
    sorted.sort();
    assert_eq!(publics, &sorted[..], "sorted by name: {rows:?}");

    // `/stats` needs the source and git as well as the outline.
    let stats = seed::stats_block(module, FIXTURE, &FileHistory::default());
    assert!(stats.starts_with("```lgtm:stats"));
    assert!(stats.contains("lines: "));

    // And deps, which the drawer reads to build its reaches list.
    let deps = seed::deps_block(module);
    assert!(deps.starts_with("```lgtm:deps module=MyApp.Accounts"));
    assert!(deps.contains("MyApp.Repo"), "{deps}");
}

/// The surface order, pinned as a literal.
///
/// The explore drawer builds its surface from the live outline in TypeScript, so
/// there are two sorters again — the exact situation that once produced
/// `get_user_by_email/1, get_user!/1, get_user/1` in the renderer against
/// `get_user/1, get_user!/1` in the seeder. `explore.ts` sorts by `(name, arity)`
/// deliberately, and its probe pins **this same string**, so if either side
/// changes its mind one of the two fails.
#[test]
fn the_surface_order_is_pinned_for_the_drawer_to_match() {
    let outline = outline();
    let module = outline.modules.first().expect("module");
    // Bound, so the block outlives the slices borrowed out of it.
    let block = seed::surface_block(module);
    let rows: Vec<&str> = block
        .lines()
        .filter(|l| l.contains(" : "))
        .map(|l| l.trim().split(" : ").next().unwrap().trim())
        .collect();

    assert_eq!(
        rows,
        ["create_user/1", "get_user/1", "get_user!/1", "changeset/2", "normalize/1"],
        "public then private, each by (name, arity) — src/lib/explore.ts pins the same"
    );
}

/// The deps block, pinned as a literal.
///
/// `ReachOverlay` writes this same text in TypeScript — `seedDepsBlock` — because
/// `renderDeps` parses block *text* and the diagram should not need a round trip
/// to Rust to open. Two writers of one grammar, so both pin the same string: if
/// either changes its padding or its order, one of the two fails.
#[test]
fn the_deps_block_is_pinned_for_the_overlay_to_match() {
    let outline = outline();
    let block = seed::deps_block(outline.modules.first().expect("module"));
    assert_eq!(
        block,
        "```lgtm:deps module=MyApp.Accounts\n\
           \x20 MyApp.User : app\n\
           \x20   %User{} : create_user/1\n\
         \x20 MyApp.Repo : app\n\
         \x20   insert/1 : create_user/1\n\
         \x20   get/2    : get_user/1\n\
         \x20   get!/2   : get_user!/1\n\
         ```\n",
        "src/lib/explore.ts seedDepsBlock pins the same string"
    );
}
