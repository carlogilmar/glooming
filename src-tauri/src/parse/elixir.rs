//! Elixir outline extraction.
//!
//! The grammar has no `function_definition` node — in tree-sitter-elixir
//! *everything* is a `call`. `def foo(x) do … end` is a call to the identifier
//! `def` whose arguments contain another call (`foo(x)`) plus a do-block. So we
//! walk for calls whose target identifier is `def`/`defp`/`defmacro`/`defmacrop`
//! and dig the name and arity out of the argument shapes:
//!
//! ```text
//! def foo(a, b) do …          call(def, args[ call(foo, args[a, b]) ], do_block)
//! def foo do …                call(def, args[ identifier(foo) ], do_block)
//! def foo(a), do: a           call(def, args[ call(foo, args[a]), keywords ])
//! def foo(a) when guard do …  call(def, args[ binary_operator(when, call(foo…)) ])
//! def foo(a, b \\ nil) do …   arity 2, min_arity 1
//! ```

use super::kinds;
use super::{Dep, DepKind, FileKind, FnInfo, ModuleInfo, Outline, Range, RemoteFn, Visibility};
use std::collections::HashMap;
use crate::error::{AppError, AppResult};
use tree_sitter::{Node, Parser};

pub fn parse(source: &str) -> AppResult<Outline> {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_elixir::LANGUAGE.into())
        .map_err(|e| AppError::Parse(format!("loading elixir grammar: {e}")))?;

    let tree = parser
        .parse(source, None)
        .ok_or_else(|| AppError::Parse("tree-sitter returned no tree".into()))?;

    let root = tree.root_node();

    // A config script has no modules at all, so it is decided before the walk.
    if kinds::looks_like_config(root, source) {
        return Ok(Outline {
            lang: "elixir".into(),
            kind: FileKind::Config,
            modules: Vec::new(),
            config: Some(kinds::config_info(root, source)),
            tests: None,
        });
    }

    let mut modules = Vec::new();
    collect_modules(root, source, &mut modules);

    // A test suite is a module, but a `use …Case` one — and the blocks that
    // suit a module (surface, treemap, reach) say nothing useful about it.
    if let Some(tests) = test_suite(root, source) {
        return Ok(Outline {
            lang: "elixir".into(),
            kind: FileKind::Test,
            modules,
            config: None,
            tests: Some(tests),
        });
    }

    // A module with nothing in it is not a module worth describing.
    let kind = if modules.iter().any(|m| !m.functions.is_empty()) {
        FileKind::Module
    } else {
        FileKind::Plain
    };

    Ok(Outline {
        lang: "elixir".into(),
        kind,
        modules,
        config: None,
        tests: None,
    })
}

/// The first `defmodule` whose body uses a case template.
fn test_suite(root: Node, src: &str) -> Option<super::TestInfo> {
    fn find<'a>(node: Node<'a>, src: &str) -> Option<(String, Node<'a>)> {
        if node.kind() == "call" && call_target(node, src).as_deref() == Some("defmodule") {
            let name = arguments_of(node)
                .and_then(|a| a.named_child(0))
                .map(|n| text(n, src).to_string());
            if let (Some(name), Some(body)) = (name, do_block(node)) {
                if kinds::looks_like_test(body, src) {
                    return Some((name, body));
                }
            }
        }
        let mut cursor = node.walk();
        let found = node.named_children(&mut cursor).find_map(|c| find(c, src));
        found
    }

    let (name, body) = find(root, src)?;
    Some(kinds::test_info(&name, body, src))
}

// ---------------------------------------------------------------- modules ---

/// Walk the whole tree for `defmodule` calls. Recursive so nested modules are
/// found too, even though we only expect one module per file.
fn collect_modules(node: Node, src: &str, out: &mut Vec<ModuleInfo>) {
    if node.kind() == "call" && call_target(node, src).as_deref() == Some("defmodule") {
        if let Some(m) = module_from_call(node, src) {
            out.push(m);
        }
        // Don't descend: nested modules inside this one are collected by
        // module_from_call's own scan of the body.
        return;
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_modules(child, src, out);
    }
}

fn module_from_call(node: Node, src: &str) -> Option<ModuleInfo> {
    let args = arguments_of(node)?;
    let name_node = args.named_child(0)?;
    let name = text(name_node, src).to_string();

    let body = do_block(node)?;
    let mut functions = Vec::new();
    let mut refs = Vec::new();
    collect_functions(body, src, &mut functions, &mut refs);

    let deps = resolve_deps(&alias_table(body, src), &refs, &name);

    Some(ModuleInfo {
        name,
        line: line_of(node),
        doc: attribute_text(body, src, "moduledoc"),
        doc_range: attribute_range(body, src, "moduledoc"),
        functions,
        deps,
    })
}

// ------------------------------------------------------------------ deps ---

/// A use of something outside this module, before the alias table is applied.
struct Ref {
    /// The local `name/arity` doing the calling.
    caller: String,
    /// The name as written — `Repo`, `String`, or an `as:` shorthand.
    prefix: String,
    /// `insert/1`, or `%User{}` for a struct literal.
    call: String,
}

/// Local name → full module, from the module's `alias` directives.
///
/// Three forms, all confirmed against the grammar rather than guessed:
///   alias MyApp.Repo                  args[ alias ]
///   alias MyApp.{User, Profile}       args[ dot[ alias, tuple[alias, …] ] ]
///   alias MyApp.Settings, as: S       args[ alias, keywords[ pair[…] ] ]
fn alias_table(body: Node, src: &str) -> HashMap<String, String> {
    let mut table = HashMap::new();
    let mut cursor = body.walk();

    for child in body.named_children(&mut cursor) {
        if child.kind() != "call" || call_target(child, src).as_deref() != Some("alias") {
            continue;
        }
        let Some(args) = arguments_of(child) else { continue };
        let Some(first) = args.named_child(0) else { continue };

        match first.kind() {
            // `alias MyApp.{User, Profile}` — one directive, several modules.
            "dot" => {
                let Some(base) = first.named_child(0) else { continue };
                let Some(tuple) = child_of_kind(first, "tuple") else { continue };
                let base = text(base, src);
                let mut tc = tuple.walk();
                for leaf in tuple.named_children(&mut tc) {
                    let leaf = text(leaf, src);
                    table.insert(last_segment(leaf).to_string(), format!("{base}.{leaf}"));
                }
            }
            "alias" => {
                let full = text(first, src).to_string();
                // `, as: S` renames it; otherwise the last segment is the name.
                let local = alias_as(args, src).unwrap_or_else(|| last_segment(&full).to_string());
                table.insert(local, full);
            }
            _ => {}
        }
    }
    table
}

/// The `as: S` half of an alias directive.
fn alias_as(args: Node, src: &str) -> Option<String> {
    let keywords = child_of_kind(args, "keywords")?;
    let pair = child_of_kind(keywords, "pair")?;
    let key = pair.named_child(0)?;
    if !text(key, src).starts_with("as") {
        return None;
    }
    Some(text(pair.named_child(1)?, src).to_string())
}

fn last_segment(module: &str) -> &str {
    module.rsplit('.').next().unwrap_or(module)
}

/// Elixir's own standard library — the part nobody needs pointed out.
const STDLIB: [&str; 46] = [
    "Kernel", "String", "Enum", "Map", "List", "Keyword", "Tuple", "Atom", "Integer", "Float",
    "Range", "Stream", "Task", "Agent", "GenServer", "Supervisor", "DynamicSupervisor", "Registry",
    "Process", "Node", "Port", "Agent", "File", "IO", "Path", "System", "Code", "Module", "Macro",
    "Regex", "Date", "Time", "DateTime", "NaiveDateTime", "Calendar", "URI", "Base", "Bitwise",
    "Access", "Application", "Config", "Exception", "Logger", "Protocol", "Record", "Version",
];

fn classify(module: &str, current: &str) -> DepKind {
    let root = |m: &str| m.split('.').next().unwrap_or(m).to_string();
    if root(module) == root(current) {
        DepKind::App
    } else if STDLIB.contains(&root(module).as_str()) {
        DepKind::Std
    } else {
        DepKind::Lib
    }
}

/// Turn raw references into the dependency list, resolving each prefix through
/// the alias table.
///
/// **Only aliased modules count.** A bare `String.trim/1` or `Enum.map/2` is a
/// call, not a declared dependency — the `alias` list at the top of the file is
/// what the author chose to depend on, and drowning that in stdlib noise buries
/// the one thing worth seeing.
fn resolve_deps(table: &HashMap<String, String>, refs: &[Ref], current: &str) -> Vec<Dep> {
    let mut deps: Vec<Dep> = Vec::new();

    for r in refs {
        let Some(module) = table.get(&r.prefix).cloned() else {
            continue;
        };

        // The module itself is not a dependency of itself.
        if module == current {
            continue;
        }

        let dep = match deps.iter_mut().find(|d| d.module == module) {
            Some(d) => d,
            None => {
                deps.push(Dep {
                    kind: classify(&module, current),
                    module,
                    functions: Vec::new(),
                });
                deps.last_mut().expect("just pushed")
            }
        };

        match dep.functions.iter_mut().find(|f| f.name == r.call) {
            Some(f) => {
                if !f.callers.contains(&r.caller) {
                    f.callers.push(r.caller.clone());
                }
            }
            None => dep.functions.push(RemoteFn {
                name: r.call.clone(),
                callers: vec![r.caller.clone()],
            }),
        }
    }
    deps
}

/// Collect every reference to something outside the module from one function's
/// body: qualified calls (`Repo.insert(cs)`) and struct literals (`%User{}`).
fn collect_refs(node: Node, src: &str, caller: &str, out: &mut Vec<Ref>) {
    match node.kind() {
        // `Repo.insert(changeset)` — a call whose target is a dot.
        "call" => {
            if let Some(dot) = child_of_kind(node, "dot") {
                if let (Some(left), Some(right)) = (dot.named_child(0), dot.named_child(1)) {
                    if left.kind() == "alias" && right.kind() == "identifier" {
                        out.push(Ref {
                            caller: caller.to_string(),
                            prefix: text(left, src).to_string(),
                            call: format!("{}/{}", text(right, src), call_arity(node, src)),
                        });
                    }
                }
            }
        }
        // `%User{}` — a struct literal is a dependency on the struct's module.
        "struct" => {
            if let Some(name) = node.named_child(0) {
                if name.kind() == "alias" {
                    out.push(Ref {
                        caller: caller.to_string(),
                        prefix: text(name, src).to_string(),
                        call: format!("%{}{{}}", text(name, src)),
                    });
                }
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        collect_refs(child, src, caller, out);
    }
}

/// Arity of a remote call, **accounting for the pipe**.
///
/// `attrs |> Repo.insert()` is written with no arguments but calls `insert/1`:
/// the pipe passes the left-hand side as the first argument. Reporting
/// `insert/0` would name a function that does not exist.
fn call_arity(node: Node, src: &str) -> usize {
    let written = arguments_of(node)
        .map(|a| {
            let mut c = a.walk();
            a.named_children(&mut c).count()
        })
        .unwrap_or(0);

    let piped = node
        .parent()
        .filter(|p| p.kind() == "binary_operator")
        .filter(|p| operator_text(*p, src).as_deref() == Some("|>"))
        // Only the right-hand side of a pipe receives the piped value.
        .and_then(|p| p.named_child(1))
        .map(|right| right.id() == node.id())
        .unwrap_or(false);

    written + usize::from(piped)
}

// -------------------------------------------------------------- functions ---

const DEF_KEYWORDS: [&str; 4] = ["def", "defp", "defmacro", "defmacrop"];

/// Collect definitions from a module body. Clauses of the same `name/arity`
/// collapse into one entry, counted — one row per function is what a reader
/// wants, not one per clause.
fn collect_functions(body: Node, src: &str, out: &mut Vec<FnInfo>, refs: &mut Vec<Ref>) {
    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        // A def can be wrapped in a block, or sit directly in the do_block.
        if child.kind() == "call" {
            if let Some(f) = fn_from_call(child, src) {
                // Walk this clause for what it reaches while we know whose it is.
                collect_refs(child, src, &f.signature(), refs);
                push_clause(out, f);
                continue;
            }
        }
        // Anything else that might contain defs (a `block`, an `if`, …).
        if child.kind() == "block" {
            collect_functions(child, src, out, refs);
        }
    }
}

/// Merge a freshly-parsed clause into the accumulating list.
fn push_clause(out: &mut Vec<FnInfo>, f: FnInfo) {
    if let Some(existing) = out
        .iter_mut()
        .find(|e| e.name == f.name && e.arity == f.arity && e.visibility == f.visibility)
    {
        existing.clauses = existing.clauses.saturating_add(1);
        // Keep the first clause's position for jumping, but remember every
        // clause so selecting the row can highlight all of them.
        existing.clause_ranges.extend(f.clause_ranges);
        // A later clause may carry the @doc / @spec the first one lacked.
        if existing.doc.is_none() {
            existing.doc = f.doc;
            existing.doc_range = existing.doc_range.or(f.doc_range);
        }
        if existing.spec_range.is_none() {
            existing.spec_range = f.spec_range;
        }
        return;
    }
    out.push(f);
}

fn fn_from_call(node: Node, src: &str) -> Option<FnInfo> {
    let keyword = call_target(node, src)?;
    if !DEF_KEYWORDS.contains(&keyword.as_str()) {
        return None;
    }
    let visibility = if keyword.ends_with('p') && keyword != "def" {
        Visibility::Private
    } else {
        Visibility::Public
    };

    let args = arguments_of(node)?;
    let mut head = args.named_child(0)?;

    // `def foo(a) when is_x(a)` — the head hides on the left of the `when`.
    if head.kind() == "binary_operator" {
        head = field_or_nth(head, "left", 0).unwrap_or(head);
    }

    let (name, arity, min_arity) = match head.kind() {
        // `def foo(a, b)` — a nested call carries the name and the arg list.
        "call" => {
            let name = call_target(head, src)?;
            let (arity, defaults) = count_args(head, src);
            (name, arity, arity.saturating_sub(defaults))
        }
        // `def foo do` — no parens, no args.
        "identifier" => (text(head, src).to_string(), 0, 0),
        // Operator definitions (`def a + b`) and anything exotic: skip rather
        // than guess. Better a missing row than a wrong one.
        _ => return None,
    };

    let attrs = preceding_attrs(node, src);
    let range = Range::of(node);

    Some(FnInfo {
        name,
        arity,
        min_arity,
        visibility,
        line: range.start,
        end_line: range.end,
        clauses: 1,
        clause_ranges: vec![range],
        doc: attrs.doc,
        doc_range: attrs.doc_range,
        spec_range: attrs.spec_range,
    })
}

/// Returns (arity, number of arguments with defaults). `def f(a, b \\ nil)` is
/// callable as both `f/1` and `f/2`, which the UI renders as `f/1..2`.
fn count_args(head: Node, src: &str) -> (u8, u8) {
    let Some(args) = arguments_of(head) else {
        return (0, 0);
    };
    let mut arity = 0u8;
    let mut defaults = 0u8;
    let mut cursor = args.walk();
    for arg in args.named_children(&mut cursor) {
        // Trailing `opts \\ []` keyword lists are still one argument.
        arity = arity.saturating_add(1);
        if arg.kind() == "binary_operator" && operator_text(arg, src).as_deref() == Some("\\\\") {
            defaults = defaults.saturating_add(1);
        }
    }
    (arity, defaults)
}

// ------------------------------------------------------------ module attrs ---

/// `@moduledoc """…"""` — the first matching attribute in a body.
fn attribute_text(body: Node, src: &str, attr: &str) -> Option<String> {
    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        if let Some(t) = attribute_value(child, src, attr) {
            return Some(t);
        }
        if child.kind() == "block" {
            if let Some(t) = attribute_text(child, src, attr) {
                return Some(t);
            }
        }
    }
    None
}

/// The attribute block sitting immediately above a definition.
///
/// Walks back through consecutive `@…` attributes — a definition is commonly
/// preceded by `@doc`, `@spec`, `@impl` in any order — and picks out the two
/// the UI cares about. Stops at the first non-attribute, since anything else
/// means the attributes belong to something further up.
#[derive(Default)]
struct Attrs {
    doc: Option<String>,
    doc_range: Option<Range>,
    spec_range: Option<Range>,
}

fn preceding_attrs(node: Node, src: &str) -> Attrs {
    let mut attrs = Attrs::default();
    let mut prev = node.prev_named_sibling();

    while let Some(p) = prev {
        let Some(name) = attribute_name(p, src) else {
            break;
        };
        match name.as_str() {
            "doc" => {
                if attrs.doc.is_none() {
                    attrs.doc = attribute_value(p, src, "doc");
                    attrs.doc_range = Some(Range::of(p));
                }
            }
            "spec" => {
                // Several @specs can stack for multi-clause functions; the one
                // nearest the definition is the one to show.
                if attrs.spec_range.is_none() {
                    attrs.spec_range = Some(Range::of(p));
                }
            }
            _ => {}
        }
        prev = p.prev_named_sibling();
    }
    attrs
}

/// Where a named attribute sits in a body, for styling.
fn attribute_range(body: Node, src: &str, attr: &str) -> Option<Range> {
    let mut cursor = body.walk();
    let found = body
        .named_children(&mut cursor)
        .find(|c| attribute_name(*c, src).as_deref() == Some(attr))
        .map(Range::of);
    found
}

/// The attribute name of an `@foo …` node, if this node is one.
fn attribute_name(node: Node, src: &str) -> Option<String> {
    if node.kind() != "unary_operator" {
        return None;
    }
    let operand = field_or_nth(node, "operand", 0)?;
    match operand.kind() {
        // `@doc "text"` — a call: target `doc`, arguments the value.
        "call" => call_target(operand, src),
        // `@required [:a]` where the grammar sees a bare identifier.
        "identifier" => Some(text(operand, src).to_string()),
        _ => None,
    }
}

fn attribute_value(node: Node, src: &str, attr: &str) -> Option<String> {
    if attribute_name(node, src).as_deref() != Some(attr) {
        return None;
    }
    let operand = field_or_nth(node, "operand", 0)?;
    let args = arguments_of(operand)?;
    let value = args.named_child(0)?;
    Some(unquote(text(value, src)))
}

/// Strip `"""` / `"` fencing and the common leading indent from a doc string.
fn unquote(raw: &str) -> String {
    let body = raw
        .trim()
        .trim_start_matches("~S")
        .trim_start_matches("\"\"\"")
        .trim_end_matches("\"\"\"")
        .trim_matches('"');
    let indent = body
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.len() - l.trim_start().len())
        .min()
        .unwrap_or(0);
    body.lines()
        .map(|l| if l.len() >= indent { &l[indent..] } else { l })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// ---------------------------------------------------------------- helpers ---

fn text<'a>(node: Node, src: &'a str) -> &'a str {
    &src[node.byte_range()]
}

fn line_of(node: Node) -> u32 {
    node.start_position().row as u32 + 1
}

/// First named child of a given kind.
///
/// Necessary because the grammar names only *some* children: a `call` exposes
/// `target` as a field but its argument list is a plain `arguments` child with
/// no field name at all, so `child_by_field_name("arguments")` is always None.
/// Everything here goes through kind lookups with field lookups as a fallback.
fn child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let found = node.named_children(&mut cursor).find(|c| c.kind() == kind);
    found
}

fn field_or_nth<'a>(node: Node<'a>, field: &str, n: usize) -> Option<Node<'a>> {
    node.child_by_field_name(field)
        .or_else(|| node.named_child(n))
}

/// The argument list of a `call`.
fn arguments_of(node: Node) -> Option<Node> {
    child_of_kind(node, "arguments")
}

/// The identifier a `call` node targets, e.g. `def` or `create_user`.
fn call_target(node: Node, src: &str) -> Option<String> {
    let target = field_or_nth(node, "target", 0)?;
    match target.kind() {
        "identifier" | "alias" => Some(text(target, src).to_string()),
        _ => None,
    }
}

/// The operator token of a `binary_operator`. Operators are anonymous nodes, so
/// their *kind* is the literal symbol (`\\`, `when`, `|>`).
fn operator_text(node: Node, src: &str) -> Option<String> {
    if let Some(op) = node.child_by_field_name("operator") {
        return Some(text(op, src).to_string());
    }
    let mut cursor = node.walk();
    let found = node
        .children(&mut cursor)
        .find(|c| !c.is_named())
        .map(|c| c.kind().to_string());
    found
}

/// The `do … end` block hanging off a call.
fn do_block(node: Node) -> Option<Node> {
    let mut cursor = node.walk();
    let found = node
        .named_children(&mut cursor)
        .find(|c| c.kind() == "do_block");
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"defmodule MyApp.Accounts do
  @moduledoc """
  Reads and writes for the users table.
  """

  import Ecto.Changeset

  @required [:email, :name]

  @doc "Creates a user from raw attrs."
  def create_user(attrs) do
    attrs |> normalize() |> Repo.insert()
  end

  def get_user(id) do
    Repo.get(User, id)
  end

  def get_user!(id), do: Repo.get!(User, id)

  def all, do: Repo.all(User)

  def search(term, opts \\ []) do
    {term, opts}
  end

  defp normalize(%{email: email} = attrs) when is_map(attrs) do
    attrs
  end

  defp normalize(attrs), do: attrs

  defp changeset(user, attrs) do
    cast(user, attrs, @required)
  end
end
"#;

    fn outline() -> Outline {
        parse(SAMPLE).expect("parses")
    }

    fn find<'a>(o: &'a Outline, sig: &str) -> &'a FnInfo {
        o.modules[0]
            .functions
            .iter()
            .find(|f| f.signature() == sig)
            .unwrap_or_else(|| panic!("no {sig} in {:?}", sigs(o)))
    }

    fn sigs(o: &Outline) -> Vec<String> {
        o.modules[0]
            .functions
            .iter()
            .map(|f| f.signature())
            .collect()
    }

    #[test]
    fn finds_the_module() {
        let o = outline();
        assert_eq!(o.modules.len(), 1);
        assert_eq!(o.modules[0].name, "MyApp.Accounts");
        assert_eq!(o.modules[0].line, 1);
        assert!(o.modules[0]
            .doc
            .as_deref()
            .unwrap()
            .starts_with("Reads and writes"));
    }

    #[test]
    fn separates_public_from_private() {
        let o = outline();
        let pub_count = o.modules[0]
            .functions
            .iter()
            .filter(|f| f.visibility == Visibility::Public)
            .count();
        let priv_count = o.modules[0]
            .functions
            .iter()
            .filter(|f| f.visibility == Visibility::Private)
            .count();
        assert_eq!(pub_count, 5, "public: {:?}", sigs(&o));
        assert_eq!(priv_count, 2, "private: {:?}", sigs(&o));
    }

    #[test]
    fn collapses_clauses_of_one_function() {
        let o = outline();
        // normalize/1 is defined twice (guard clause + passthrough).
        assert_eq!(find(&o, "normalize/1").clauses, 2);
        assert_eq!(find(&o, "create_user/1").clauses, 1);
    }

    #[test]
    fn handles_zero_arity_and_one_liners() {
        let o = outline();
        assert_eq!(find(&o, "all/0").arity, 0);
        let bang = find(&o, "get_user!/1");
        assert_eq!(bang.name, "get_user!");
        // A `, do:` one-liner starts and ends on the same line.
        assert_eq!(bang.line, bang.end_line);
    }

    #[test]
    fn records_default_arguments_as_an_arity_range() {
        let o = outline();
        let search = find(&o, "search/2");
        assert_eq!(search.arity, 2);
        assert_eq!(search.min_arity, 1, "callable as search/1 and search/2");
    }

    #[test]
    fn attaches_doc_attributes() {
        let o = outline();
        assert_eq!(
            find(&o, "create_user/1").doc.as_deref(),
            Some("Creates a user from raw attrs.")
        );
        assert!(find(&o, "get_user/1").doc.is_none());
    }

    #[test]
    fn line_numbers_point_at_the_definition() {
        let o = outline();
        let f = find(&o, "create_user/1");
        let line = SAMPLE.lines().nth(f.line as usize - 1).unwrap();
        assert!(line.contains("def create_user"), "got: {line}");
    }

#[test]
    fn every_clause_range_is_recorded() {
        let o = outline();
        let n = o.modules[0]
            .functions
            .iter()
            .find(|f| f.signature() == "normalize/1")
            .unwrap();
        // Two clauses: the guarded one, then the passthrough one-liner.
        assert_eq!(n.clauses, 2);
        assert_eq!(n.clause_ranges.len(), 2);
        assert!(n.clause_ranges[0].start < n.clause_ranges[1].start);
        // The jump target stays the first clause.
        assert_eq!(n.line, n.clause_ranges[0].start);
    }

    const WITH_SPECS: &str = r#"defmodule MyApp.Accounts do
  @doc "Creates a user."
  @spec create_user(map()) :: {:ok, User.t()} | {:error, term()}
  def create_user(attrs) do
    attrs
  end

  @spec get_user(integer()) :: User.t() | nil
  def get_user(id), do: id

  def undocumented(x), do: x
end
"#;

    #[test]
    fn finds_the_spec_above_a_definition() {
        let o = parse(WITH_SPECS).unwrap();
        let f = |sig: &str| {
            o.modules[0]
                .functions
                .iter()
                .find(|f| f.signature() == sig)
                .unwrap_or_else(|| panic!("missing {sig}"))
        };

        // @spec sits on line 3, the def on line 4 — the doc is above the spec,
        // so both must be found by walking back through the attribute block.
        let create = f("create_user/1");
        assert_eq!(create.spec_range.map(|r| r.start), Some(3));
        assert_eq!(create.doc_range.map(|r| r.start), Some(2));
        assert_eq!(create.doc.as_deref(), Some("Creates a user."));

        // A spec with no doc above it.
        assert_eq!(f("get_user/1").spec_range.map(|r| r.start), Some(8));
        assert!(f("get_user/1").doc_range.is_none());

        // Neither.
        assert!(f("undocumented/1").spec_range.is_none());
        assert!(f("undocumented/1").doc_range.is_none());
    }

    #[test]
    fn records_where_the_moduledoc_sits() {
        let o = outline();
        let r = o.modules[0].doc_range.expect("moduledoc range");
        // The heredoc spans lines 2-4 of the fixture.
        assert_eq!((r.start, r.end), (2, 4));
    }

    const DEPS_SAMPLE: &str = r#"defmodule MyApp.Accounts do
  alias MyApp.Repo
  alias MyApp.{User, Profile}
  alias MyApp.Accounts.Settings, as: S
  alias MyApp.Unused
  alias Ecto.Changeset

  def create_user(attrs) do
    %User{}
    |> Repo.insert()
  end

  def get_user(id), do: Repo.get(User, id)

  def touch, do: S.touch(1, 2)

  defp normalize(a), do: Changeset.cast(a, %{}, [])

  defp pure(a), do: a
end
"#;

    fn deps() -> Vec<crate::parse::Dep> {
        parse(DEPS_SAMPLE).unwrap().modules[0].deps.clone()
    }

    fn dep<'a>(all: &'a [crate::parse::Dep], module: &str) -> &'a crate::parse::Dep {
        all.iter()
            .find(|d| d.module == module)
            .unwrap_or_else(|| panic!("no {module} in {:?}", all.iter().map(|d| &d.module).collect::<Vec<_>>()))
    }

    #[test]
    fn resolves_every_alias_form() {
        let all = deps();
        // Plain, multi-alias braces, and an `as:` rename all resolve to full names.
        assert_eq!(dep(&all, "MyApp.Repo").module, "MyApp.Repo");
        assert_eq!(dep(&all, "MyApp.User").module, "MyApp.User");
        assert_eq!(dep(&all, "MyApp.Accounts.Settings").module, "MyApp.Accounts.Settings");
        // An alias that is never used is not a dependency.
        assert!(all.iter().all(|d| d.module != "MyApp.Unused"));
    }

    #[test]
    fn a_pipe_adds_one_to_the_arity() {
        let all = deps();
        let repo = dep(&all, "MyApp.Repo");
        let names: Vec<&str> = repo.functions.iter().map(|f| f.name.as_str()).collect();
        // `attrs |> Repo.insert()` is written with no arguments but calls insert/1.
        assert!(names.contains(&"insert/1"), "pipe arity: {names:?}");
        assert!(!names.contains(&"insert/0"), "insert/0 does not exist: {names:?}");
        // An unpiped call is counted as written.
        assert!(names.contains(&"get/2"), "{names:?}");
    }

    #[test]
    fn attributes_each_call_to_its_function() {
        let all = deps();
        let insert = dep(&all, "MyApp.Repo")
            .functions
            .iter()
            .find(|f| f.name == "insert/1")
            .unwrap();
        assert_eq!(insert.callers, vec!["create_user/1"]);

        let cast = dep(&all, "Ecto.Changeset")
            .functions
            .iter()
            .find(|f| f.name == "cast/3")
            .unwrap();
        assert_eq!(cast.callers, vec!["normalize/1"]);
    }

    #[test]
    fn struct_literals_count_as_dependencies() {
        let all = deps();
        let user = dep(&all, "MyApp.User");
        let names: Vec<&str> = user.functions.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"%User{}"), "{names:?}");
        // …and Repo.get(User, id) passes the module itself, which is not a call
        // on it, so User has exactly the struct plus nothing invented.
        assert_eq!(user.functions.len(), 1, "{names:?}");
    }

    #[test]
    fn classifies_by_distance_from_this_module() {
        let all = deps();
        assert_eq!(dep(&all, "MyApp.Repo").kind, crate::parse::DepKind::App);
        assert_eq!(dep(&all, "Ecto.Changeset").kind, crate::parse::DepKind::Lib);
    }

    #[test]
    fn only_aliased_modules_are_dependencies() {
        // String and Enum are called but never aliased: they are calls, not the
        // module's declared surface, and listing them buries the real ones.
        let o = parse(
            "defmodule MyApp.A do\n  alias MyApp.Repo\n\
               def f(a), do: a |> String.trim() |> Enum.count() |> Repo.insert()\nend",
        )
        .unwrap();
        let names: Vec<&str> = o.modules[0].deps.iter().map(|d| d.module.as_str()).collect();
        assert_eq!(names, vec!["MyApp.Repo"], "{names:?}");
    }

    #[test]
    fn a_module_is_not_its_own_dependency() {
        let o = parse(
            "defmodule MyApp.A do\n  def f, do: MyApp.A.g()\n  def g, do: 1\nend",
        )
        .unwrap();
        assert!(o.modules[0].deps.is_empty(), "{:?}", o.modules[0].deps);
    }

    #[test]
    fn empty_and_moduleless_files_do_not_panic() {
        assert_eq!(parse("").unwrap().modules.len(), 0);
        assert_eq!(parse("x = 1\n").unwrap().modules.len(), 0);
    }
}



