//! Config files and test suites.
//!
//! A module is not the only shape an Elixir file comes in, and the blocks that
//! suit one make no sense for another. These two kinds have enough regular
//! structure to be worth extracting; everything else falls through to `Plain`
//! and gets a blank page.
//!
//! Node shapes, all confirmed by dumping the tree rather than guessing:
//!
//! ```text
//! config :app, k: v            call[ identifier(config), args[ atom, keywords ] ]
//! config :app, Target, k: v    call[ identifier(config), args[ atom, alias|atom, keywords ] ]
//! System.get_env("X")          call[ dot[ alias(System), identifier ], args[ string ] ]
//! import_config "dev.exs"      call[ identifier(import_config), args[ string ] ]
//!
//! use MyApp.DataCase, async: true   call[ identifier(use), args[ alias, keywords ] ]
//! describe "name" do … end          call[ identifier(describe), args[ string ], do_block ]
//! test "name", %{a: b} do … end     call[ identifier(test), args[ string, map ], do_block ]
//! setup do … end                    call[ identifier(setup), do_block ]
//! setup :named                      call[ identifier(setup), args[ atom ] ]
//! @tag :slow                        unary_operator[ call[ identifier(tag), args[ atom ] ] ]
//! ```

use super::{
    ConfigGroup, ConfigInfo, Describe, SetupInfo, Setting, TestCase, TestInfo, ValueSource,
};
use tree_sitter::Node;

// Shared with elixir.rs — kept private there, re-declared here as thin helpers.
fn text<'a>(node: Node, src: &'a str) -> &'a str {
    &src[node.byte_range()]
}

fn line_of(node: Node) -> u32 {
    node.start_position().row as u32 + 1
}

fn end_line_of(node: Node) -> u32 {
    node.end_position().row as u32 + 1
}

fn child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let found = node.named_children(&mut cursor).find(|c| c.kind() == kind);
    found
}

fn arguments_of(node: Node) -> Option<Node> {
    child_of_kind(node, "arguments")
}

fn do_block(node: Node) -> Option<Node> {
    child_of_kind(node, "do_block")
}

/// The identifier a `call` targets — `config`, `describe`, `setup`.
fn call_name(node: Node, src: &str) -> Option<String> {
    if node.kind() != "call" {
        return None;
    }
    let target = node.named_child(0)?;
    (target.kind() == "identifier").then(|| text(target, src).to_string())
}

/// The text inside a `string` node, without its quotes.
fn string_value(node: Node, src: &str) -> Option<String> {
    if node.kind() != "string" {
        return None;
    }
    Some(
        child_of_kind(node, "quoted_content")
            .map(|c| text(c, src).to_string())
            .unwrap_or_else(|| text(node, src).trim_matches('"').to_string()),
    )
}

/// `keyword: ` → `keyword`. The grammar keeps the colon and trailing space.
fn keyword_name(pair: Node, src: &str) -> Option<String> {
    let key = pair.named_child(0)?;
    Some(text(key, src).trim().trim_end_matches(':').to_string())
}

fn pairs_of(keywords: Node) -> Vec<Node> {
    let mut cursor = keywords.walk();
    let found: Vec<Node> = keywords
        .named_children(&mut cursor)
        .filter(|c| c.kind() == "pair")
        .collect();
    found
}

// ---------------------------------------------------------------- detection ---

/// Is this a config script? `import Config` (or the deprecated `use Mix.Config`)
/// is the reliable marker; living under `config/` is a weaker hint used by the
/// caller when the file is empty of both.
pub fn looks_like_config(root: Node, src: &str) -> bool {
    let mut cursor = root.walk();
    for child in root.named_children(&mut cursor) {
        match call_name(child, src).as_deref() {
            Some("import") | Some("use") => {
                if let Some(args) = arguments_of(child) {
                    let t = text(args, src);
                    if t == "Config" || t.starts_with("Mix.Config") {
                        return true;
                    }
                }
            }
            Some("config") | Some("import_config") => return true,
            _ => {}
        }
    }
    false
}

/// Is this module a test suite? Any `use …Case` / `use ExUnit.Case` in the body.
pub fn looks_like_test(body: Node, src: &str) -> bool {
    let mut cursor = body.walk();
    let found = body.named_children(&mut cursor).any(|child| {
        call_name(child, src).as_deref() == Some("use")
            && arguments_of(child)
                .map(|a| {
                    let t = text(a, src);
                    t.starts_with("ExUnit.Case") || t.split(',').next().unwrap_or("").contains("Case")
                })
                .unwrap_or(false)
    });
    found
}

// ------------------------------------------------------------------- config ---

pub fn config_info(root: Node, src: &str) -> ConfigInfo {
    let mut info = ConfigInfo::default();
    let mut cursor = root.walk();

    for child in root.named_children(&mut cursor) {
        match call_name(child, src).as_deref() {
            Some("config") => {
                if let Some(group) = config_group(child, src) {
                    info.groups.push(group);
                }
            }
            Some("import_config") => {
                if let Some(args) = arguments_of(child) {
                    if let Some(first) = args.named_child(0) {
                        // `import_config "#{config_env()}.exs"` is interpolated
                        // and has no literal value; record it as written.
                        let name = string_value(first, src)
                            .unwrap_or_else(|| text(first, src).trim_matches('"').to_string());
                        info.imports.push(name);
                    }
                }
            }
            _ => {}
        }
    }
    info
}

fn config_group(node: Node, src: &str) -> Option<ConfigGroup> {
    let args = arguments_of(node)?;
    let app_node = args.named_child(0)?;
    if app_node.kind() != "atom" {
        return None;
    }
    let app = text(app_node, src).to_string();

    // The second argument is either the target (three-arity form) or the
    // keyword list itself (two-arity form).
    let second = args.named_child(1)?;
    let (target, keywords) = if second.kind() == "keywords" {
        (None, second)
    } else {
        (
            Some(text(second, src).to_string()),
            child_of_kind(args, "keywords")?,
        )
    };

    let settings = pairs_of(keywords)
        .into_iter()
        .filter_map(|pair| {
            let key = keyword_name(pair, src)?;
            let value = pair.named_child(1)?;
            Some(Setting {
                line: line_of(pair),
                end_line: end_line_of(pair),
                source: value_source(&key, value, src),
                key,
            })
        })
        .collect();

    Some(ConfigGroup {
        app,
        target,
        line: line_of(node),
        end_line: end_line_of(node),
        settings,
    })
}

/// Names that mean a literal here is a credential. Matched loosely on purpose —
/// a false positive costs a hidden value, a false negative leaks one.
const SECRETISH: [&str; 8] = [
    "secret", "password", "token", "api_key", "private_key", "salt", "credential", "passwd",
];

fn value_source(key: &str, value: Node, src: &str) -> ValueSource {
    // `System.get_env("X")` / `System.fetch_env!("X")`
    if value.kind() == "call" {
        if let Some(dot) = child_of_kind(value, "dot") {
            let owner = dot.named_child(0).map(|n| text(n, src)).unwrap_or("");
            let func = dot.named_child(1).map(|n| text(n, src)).unwrap_or("");
            if owner == "System" && (func.starts_with("get_env") || func.starts_with("fetch_env")) {
                let var = arguments_of(value)
                    .and_then(|a| a.named_child(0))
                    .and_then(|n| string_value(n, src))
                    .unwrap_or_default();
                return ValueSource::Env {
                    var,
                    required: func.ends_with('!'),
                };
            }
        }
    }

    let lower = key.to_lowercase();
    if SECRETISH.iter().any(|s| lower.contains(s)) {
        return ValueSource::Secret;
    }

    // Keep literals to one line: a multi-line list would break the block's
    // one-row-per-setting shape.
    let raw = text(value, src);
    let flat = raw.split('\n').next().unwrap_or(raw).trim().to_string();
    ValueSource::Literal {
        value: if flat.len() < raw.trim().len() {
            format!("{flat} …")
        } else {
            flat
        },
    }
}

// --------------------------------------------------------------------- test ---

pub fn test_info(module: &str, body: Node, src: &str) -> TestInfo {
    let mut info = TestInfo {
        module: module.to_string(),
        ..Default::default()
    };

    // Tests written straight in the module body, with no describe around them.
    let mut loose = Describe {
        name: None,
        line: 0,
        end_line: 0,
        setups: Vec::new(),
        tests: Vec::new(),
    };
    let mut pending_tags: Vec<String> = Vec::new();

    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        // `@tag :slow` applies to the next test.
        if let Some(tag) = attribute_tag(child, src) {
            pending_tags.push(tag);
            continue;
        }

        match call_name(child, src).as_deref() {
            Some("use") => {
                if let Some(args) = arguments_of(child) {
                    if let Some(first) = args.named_child(0) {
                        info.case_template = Some(text(first, src).to_string());
                    }
                    if let Some(kw) = child_of_kind(args, "keywords") {
                        for pair in pairs_of(kw) {
                            if keyword_name(pair, src).as_deref() == Some("async") {
                                info.is_async = pair
                                    .named_child(1)
                                    .map(|v| text(v, src) == "true")
                                    .unwrap_or(false);
                            }
                        }
                    }
                }
            }
            Some(k @ ("setup" | "setup_all")) => info.setups.push(setup_info(k, child, src)),
            Some("describe") => {
                if let Some(d) = describe(child, src) {
                    info.describes.push(d);
                }
                pending_tags.clear();
            }
            Some("test") => {
                if let Some(t) = test_case(child, src, std::mem::take(&mut pending_tags)) {
                    if loose.line == 0 {
                        loose.line = t.line;
                    }
                    loose.end_line = t.end_line;
                    loose.tests.push(t);
                }
            }
            _ => pending_tags.clear(),
        }
    }

    if !loose.tests.is_empty() {
        info.describes.push(loose);
    }
    info
}

/// `@tag :slow` / `@moduletag :skip` → the tag name.
fn attribute_tag(node: Node, src: &str) -> Option<String> {
    if node.kind() != "unary_operator" {
        return None;
    }
    let operand = node.named_child(0)?;
    let name = call_name(operand, src)?;
    if name != "tag" && name != "moduletag" {
        return None;
    }
    let args = arguments_of(operand)?;
    let first = args.named_child(0)?;
    Some(text(first, src).trim_start_matches(':').to_string())
}

fn setup_info(kind: &str, node: Node, src: &str) -> SetupInfo {
    // `setup :put_user` — a callback defined elsewhere, so its keys are unknown.
    let named = arguments_of(node)
        .and_then(|a| a.named_child(0))
        .filter(|n| n.kind() == "atom")
        .map(|n| text(n, src).trim_start_matches(':').to_string());

    let provides = if named.is_some() {
        None
    } else {
        do_block(node).and_then(|b| context_keys(b, src))
    };

    SetupInfo {
        kind: kind.to_string(),
        line: line_of(node),
        end_line: end_line_of(node),
        named,
        provides,
    }
}

/// The context keys a setup block hands to its tests, read from the block's
/// **last expression**: `%{user: user}` or `{:ok, repo: repo}`.
///
/// Best-effort by nature. Anything else — a bare `:ok`, a function call, a
/// conditional — returns `None` for "unknown", which the UI shows as `+?`
/// rather than pretending the list is complete.
fn context_keys(block: Node, src: &str) -> Option<Vec<String>> {
    let mut cursor = block.walk();
    let last = block.named_children(&mut cursor).last()?;

    let keywords = match last.kind() {
        // %{user: user}
        "map" => child_of_kind(last, "map_content").and_then(|c| child_of_kind(c, "keywords")),
        // {:ok, repo: repo}
        "tuple" => child_of_kind(last, "keywords"),
        _ => None,
    }?;

    let keys: Vec<String> = pairs_of(keywords)
        .into_iter()
        .filter_map(|p| keyword_name(p, src))
        .collect();
    (!keys.is_empty()).then_some(keys)
}

fn describe(node: Node, src: &str) -> Option<Describe> {
    let name = arguments_of(node)
        .and_then(|a| a.named_child(0))
        .and_then(|n| string_value(n, src))?;

    let mut out = Describe {
        name: Some(name),
        line: line_of(node),
        end_line: end_line_of(node),
        setups: Vec::new(),
        tests: Vec::new(),
    };

    let Some(body) = do_block(node) else {
        return Some(out);
    };
    let mut pending_tags: Vec<String> = Vec::new();
    let mut cursor = body.walk();

    for child in body.named_children(&mut cursor) {
        if let Some(tag) = attribute_tag(child, src) {
            pending_tags.push(tag);
            continue;
        }
        match call_name(child, src).as_deref() {
            Some(k @ ("setup" | "setup_all")) => out.setups.push(setup_info(k, child, src)),
            Some("test") => {
                if let Some(t) = test_case(child, src, std::mem::take(&mut pending_tags)) {
                    out.tests.push(t);
                }
            }
            _ => pending_tags.clear(),
        }
    }
    Some(out)
}

fn test_case(node: Node, src: &str, tags: Vec<String>) -> Option<TestCase> {
    let name = arguments_of(node)
        .and_then(|a| a.named_child(0))
        .and_then(|n| string_value(n, src))?;

    Some(TestCase {
        line: line_of(node),
        end_line: end_line_of(node),
        // Counted over the whole call, not just its do_block: a one-liner
        // written `test "x", do: assert(y)` has no do_block at all, and used to
        // come back as zero assertions.
        asserts: count_asserts(node, src),
        skipped: tags.iter().any(|t| t == "skip"),
        tags,
        name,
    })
}

/// Every `assert`, `refute`, `assert_raise`, `assert_receive`… in a test body.
/// A rough measure of how much a test actually checks, which is the point: a
/// one-assertion test looks different from a five-assertion one.
fn count_asserts(node: Node, src: &str) -> u32 {
    let mut n = 0;
    if let Some(name) = call_name(node, src) {
        if name.starts_with("assert") || name.starts_with("refute") {
            n += 1;
        }
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        n += count_asserts(child, src);
    }
    n
}

#[cfg(test)]
mod tests {
    use crate::parse::{self, FileKind, ValueSource};

    const CONFIG: &str = r#"import Config

config :my_app, ecto_repos: [MyApp.Repo]

config :my_app, MyApp.Repo,
  username: "postgres",
  password: System.get_env("DB_PASSWORD"),
  pool_size: 10

config :my_app, MyAppWeb.Endpoint,
  secret_key_base: System.fetch_env!("SECRET_KEY_BASE"),
  signing_salt: "7dF3xQ",
  debug_errors: true

config :logger, :console, level: :debug

import_config "dev.secret.exs"
"#;

    fn config() -> parse::ConfigInfo {
        let o = parse::parse(CONFIG, "elixir").unwrap();
        assert_eq!(o.kind, FileKind::Config, "detected as config");
        o.config.expect("config info")
    }

    #[test]
    fn a_config_script_is_not_mistaken_for_a_module() {
        let o = parse::parse(CONFIG, "elixir").unwrap();
        assert_eq!(o.kind, FileKind::Config);
        assert!(o.modules.is_empty());
        assert!(o.tests.is_none());
    }

    #[test]
    fn groups_by_app_and_target() {
        let c = config();
        assert_eq!(c.groups.len(), 4);
        // Two-arity form: no target.
        assert_eq!(c.groups[0].app, ":my_app");
        assert!(c.groups[0].target.is_none());
        // Three-arity, module target and atom target.
        assert_eq!(c.groups[1].target.as_deref(), Some("MyApp.Repo"));
        assert_eq!(c.groups[3].target.as_deref(), Some(":console"));
    }

    #[test]
    fn tells_environment_values_from_literals() {
        let c = config();
        let repo = &c.groups[1];
        let pw = repo.settings.iter().find(|s| s.key == "password").unwrap();
        assert_eq!(
            pw.source,
            ValueSource::Env {
                var: "DB_PASSWORD".into(),
                required: false
            }
        );

        let endpoint = &c.groups[2];
        let skb = endpoint
            .settings
            .iter()
            .find(|s| s.key == "secret_key_base")
            .unwrap();
        // fetch_env! is required — it crashes on boot when unset.
        assert_eq!(
            skb.source,
            ValueSource::Env {
                var: "SECRET_KEY_BASE".into(),
                required: true
            }
        );

        let user = repo.settings.iter().find(|s| s.key == "username").unwrap();
        assert!(matches!(user.source, ValueSource::Literal { .. }));
    }

    #[test]
    fn masks_a_hardcoded_credential_but_still_reports_it() {
        let c = config();
        let salt = c.groups[2]
            .settings
            .iter()
            .find(|s| s.key == "signing_salt")
            .unwrap();
        // The key is listed — that it is hardcoded is the finding — but the
        // value never leaves the file.
        assert_eq!(salt.source, ValueSource::Secret);
        let json = serde_json::to_string(&salt.source).unwrap();
        assert!(!json.contains("7dF3xQ"), "value must not ride along: {json}");
    }

    #[test]
    fn records_the_load_chain() {
        assert_eq!(config().imports, vec!["dev.secret.exs"]);
    }

    const TEST_FILE: &str = r#"defmodule MyApp.AccountsTest do
  use MyApp.DataCase, async: true

  setup_all do
    {:ok, repo: Repo}
  end

  setup do
    %{user: user_fixture()}
  end

  describe "create_user/1" do
    setup :put_user

    @tag :slow
    test "creates a user", %{user: user} do
      assert user
      refute user.admin
      assert_raise ArgumentError, fn -> nil end
    end

    test "thin one" do
      assert true
    end
  end

  describe "delete_user/1" do
    setup do
      %{account: account_fixture(), session: session_fixture()}
    end

    @tag :skip
    test "emits telemetry" do
      assert true
    end
  end

  test "loose test" do
    assert true
  end
end
"#;

    fn suite() -> parse::TestInfo {
        let o = parse::parse(TEST_FILE, "elixir").unwrap();
        assert_eq!(o.kind, FileKind::Test, "detected as a test suite");
        o.tests.expect("test info")
    }

    #[test]
    fn a_test_suite_is_not_treated_as_a_plain_module() {
        let t = suite();
        assert_eq!(t.module, "MyApp.AccountsTest");
        assert_eq!(t.case_template.as_deref(), Some("MyApp.DataCase"));
        assert!(t.is_async);
    }

    #[test]
    fn separates_module_scope_setups_from_describe_scope() {
        let t = suite();
        // setup_all and setup at module level — every test inherits both.
        assert_eq!(t.setups.len(), 2);
        assert_eq!(t.setups[0].kind, "setup_all");
        assert_eq!(t.setups[0].provides.as_deref(), Some(&["repo".to_string()][..]));
        assert_eq!(t.setups[1].provides.as_deref(), Some(&["user".to_string()][..]));

        // …and each describe's own.
        let create = &t.describes[0];
        assert_eq!(create.setups.len(), 1);
    }

    #[test]
    fn a_named_setup_callback_is_unknown_not_empty() {
        let t = suite();
        let named = &t.describes[0].setups[0];
        assert_eq!(named.named.as_deref(), Some("put_user"));
        // The callback lives elsewhere in the file, so its keys can't be read.
        // `None` is "unknown", which the UI shows as +? rather than guessing.
        assert!(named.provides.is_none());
    }

    #[test]
    fn reads_context_keys_from_the_last_expression() {
        let t = suite();
        let delete = t.describes.iter().find(|d| d.name.as_deref() == Some("delete_user/1")).unwrap();
        assert_eq!(
            delete.setups[0].provides.as_deref(),
            Some(&["account".to_string(), "session".to_string()][..])
        );
    }

    #[test]
    fn counts_assertions_and_carries_tags() {
        let t = suite();
        let create = &t.describes[0];
        let first = &create.tests[0];
        // assert + refute + assert_raise
        assert_eq!(first.asserts, 3);
        assert_eq!(first.tags, vec!["slow"]);
        assert!(!first.skipped);

        assert_eq!(create.tests[1].asserts, 1, "a thin test reads as thin");

        let delete = t.describes.iter().find(|d| d.name.as_deref() == Some("delete_user/1")).unwrap();
        assert!(delete.tests[0].skipped, "@tag :skip marks it skipped");
    }

    #[test]
    fn a_test_outside_any_describe_still_appears() {
        let t = suite();
        let loose = t.describes.last().unwrap();
        assert!(loose.name.is_none());
        assert_eq!(loose.tests.len(), 1);
        assert_eq!(loose.tests[0].name, "loose test");
    }

    #[test]
    fn anything_unrecognised_falls_through_to_plain() {
        // A bare script: no module, no config, no tests.
        let o = parse::parse("IO.puts(\"hello\")\nx = 1\n", "elixir").unwrap();
        assert_eq!(o.kind, FileKind::Plain);
        assert!(o.config.is_none() && o.tests.is_none());

        // A module with no functions is not a module worth describing either.
        let empty = parse::parse("defmodule A do\n  @moduledoc \"hi\"\nend\n", "elixir").unwrap();
        assert_eq!(empty.kind, FileKind::Plain);
    }
}
