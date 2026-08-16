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

use super::{FnInfo, ModuleInfo, Outline, Visibility};
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

    let mut modules = Vec::new();
    collect_modules(tree.root_node(), source, &mut modules);

    Ok(Outline {
        lang: "elixir".into(),
        modules,
    })
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
    collect_functions(body, src, &mut functions);

    Some(ModuleInfo {
        name,
        line: line_of(node),
        doc: attribute_text(body, src, "moduledoc"),
        functions,
    })
}

// -------------------------------------------------------------- functions ---

const DEF_KEYWORDS: [&str; 4] = ["def", "defp", "defmacro", "defmacrop"];

/// Collect definitions from a module body. Clauses of the same `name/arity`
/// collapse into one entry, counted — one row per function is what a reader
/// wants, not one per clause.
fn collect_functions(body: Node, src: &str, out: &mut Vec<FnInfo>) {
    let mut cursor = body.walk();
    for child in body.named_children(&mut cursor) {
        // A def can be wrapped in a block, or sit directly in the do_block.
        if child.kind() == "call" {
            if let Some(f) = fn_from_call(child, src) {
                push_clause(out, f);
                continue;
            }
        }
        // Anything else that might contain defs (a `block`, an `if`, …).
        if child.kind() == "block" {
            collect_functions(child, src, out);
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
        // Keep the first clause's position, but let a later clause supply the
        // @doc if the first had none.
        if existing.doc.is_none() {
            existing.doc = f.doc;
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

    Some(FnInfo {
        name,
        arity,
        min_arity,
        visibility,
        line: line_of(node),
        end_line: node.end_position().row as u32 + 1,
        clauses: 1,
        doc: preceding_doc(node, src),
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

/// `@doc "…"` immediately above a definition, skipping blank lines.
fn preceding_doc(node: Node, src: &str) -> Option<String> {
    let mut prev = node.prev_named_sibling();
    while let Some(p) = prev {
        if let Some(t) = attribute_value(p, src, "doc") {
            return Some(t);
        }
        // Only look through other attributes (@spec, @impl); anything else
        // means the doc, if any, belongs to something else.
        if attribute_name(p, src).is_some() {
            prev = p.prev_named_sibling();
            continue;
        }
        break;
    }
    None
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
    fn empty_and_moduleless_files_do_not_panic() {
        assert_eq!(parse("").unwrap().modules.len(), 0);
        assert_eq!(parse("x = 1\n").unwrap().modules.len(), 0);
    }
}

