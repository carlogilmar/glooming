//! End-to-end check of the read → parse → seed → reconcile pipeline, run
//! against the exact file the UI mockup shows. If the app ever stops matching
//! `mockup/index.html`, this is where it shows up first.

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

#[test]
fn the_seeded_doc_is_ready_to_write_into() {
    let md = seed::seed_markdown(&outline());

    assert!(md.starts_with("# MyApp.Accounts\n"));
    assert!(md.contains("Reads and writes for the `users` table."));
    assert!(md.contains("```lgtm:functions module=MyApp.Accounts"));
    assert!(md.contains("public:") && md.contains("private:"));

    // @doc text becomes starting prose…
    assert!(md.contains("Creates a user from raw attrs."));
    // …and everything undocumented is an empty slot waiting for you.
    let empty = md
        .lines()
        .filter(|l| l.trim_start().starts_with("- ") && l.trim_end().ends_with(':'))
        .count();
    assert_eq!(empty, 3, "get_user!/1, normalize/1, changeset/2:\n{md}");
}

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

    let written = explain(
        &seed::seed_markdown(&outline()),
        "create_user/1",
        " Entry point. Validates, then inserts.",
    );
    let written = explain(&written, "changeset/2", " The one place field rules live.");
    assert!(written.contains("The one place field rules live."), "setup:\n{written}");

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
    assert!(merged.contains("The one place field rules live."));
    // The new function shows up with an empty slot.
    assert!(merged.contains("delete_user/1"));
    // The deleted one is struck through, not erased.
    assert!(merged.contains("~~get_user!/1~~"), "{merged}");
}
