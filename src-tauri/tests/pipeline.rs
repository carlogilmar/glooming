//! End-to-end check of the read → parse pipeline, run against the exact file
//! the UI mockup shows. If the app ever stops matching `mockup/index.html`, this
//! is where it shows up first.

use lgtm_lib::parse;

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

/// The suite for the fixture module, so the test-kind wire format has something
/// real behind it. Deliberately mirrors `mockup/tests.html`'s fixture: three
/// describes, one public function of `accounts.ex` that no describe names, and a
/// describe that names no function at all.
const SUITE: &str = r#"defmodule MyApp.AccountsTest do
  use MyApp.DataCase, async: false

  setup do
    %{user: user_fixture()}
  end

  describe "create_user/1" do
    setup :put_locale

    test "normalises the email", %{user: _user} do
      assert {:ok, user} = Accounts.create_user(%{})
      assert user.email == "ada@example.com"
    end

    @tag :slow
    test "survives a concurrent insert" do
      assert_receive {:inserted, _id}, 500
    end
  end

  describe "get_user/1" do
    test "returns nil for an unknown id" do
      refute Accounts.get_user(Ecto.UUID.generate())
    end
  end
end
"#;

fn suite() -> lgtm_lib::parse::TestInfo {
    lgtm_lib::parse::parse(SUITE, "elixir")
        .expect("parses")
        .tests
        .expect("a test suite")
}

/// The same rename trap as `the_wire_format_is_camel_case`, for the fields the
/// test drawer now reads. `tagRange` in particular decides whether selecting a
/// tagged test shows its `@tag` or dims it one line above.
#[test]
fn the_test_wire_format_is_camel_case() {
    let json = serde_json::to_string(&suite()).expect("serializes");

    for field in ["\"endLine\"", "\"tagRange\"", "\"moduleTags\"", "\"assertions\""] {
        assert!(json.contains(field), "missing {field} in: {json}");
    }
    for leak in ["end_line", "tag_range", "module_tags", "case_template"] {
        assert!(!json.contains(leak), "snake case leaked ({leak}): {json}");
    }
    // The kinds are what the drawer switches on, so their spelling is a contract.
    assert!(json.contains("\"kind\":\"assert\""), "got: {json}");
    assert!(json.contains("\"kind\":\"error\""), "got: {json}");
    assert!(json.contains("\"kind\":\"message\""), "got: {json}");
}

/// A test is a span, not a line. This is what the drawer needs in order to stop
/// dropping a one-line cursor on `test "…" do`.
#[test]
fn a_test_is_a_span_and_a_describe_contains_it() {
    let t = suite();
    let create = &t.describes[0];
    let first = &create.tests[0];

    assert!(first.end_line > first.line, "a test spans more than its header");
    assert!(
        create.line < first.line && create.end_line >= first.end_line,
        "the describe contains its tests: {}-{} vs {}-{}",
        create.line,
        create.end_line,
        first.line,
        first.end_line
    );

    // Every test's assertions land inside the test's own span.
    for d in &t.describes {
        for test in &d.tests {
            for a in &test.assertions {
                assert!(a.line >= test.line && a.line <= test.end_line);
            }
        }
    }
}

/// Dump the outline the frontend actually receives, for `probe` runs against
/// `explore.ts`. Ignored by default — it writes a file, it does not assert.
#[test]
#[ignore]
fn dump_suite_json() {
    let o = lgtm_lib::parse::parse(SUITE, "elixir").expect("parses");
    std::fs::write(
        "../src-tauri/target/suite.json",
        serde_json::to_string_pretty(&o).expect("serializes"),
    )
    .expect("writes");
}
