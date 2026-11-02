//! The project index: which module lives in which file.
//!
//! A gloom's references can name any module in the project, so "which file is
//! `Weather.Pipeline` in" has to be answerable for files nobody has opened yet.
//! This answers it and nothing more — module names and paths, no outlines. Only a
//! file you actually open, or one your prose names, is parsed in full.
//!
//! **Derived, never stored.** It is rebuilt when a project opens and whenever the
//! window comes back to the front, which is the same bargain the file picker
//! already makes: the walk is fast, and caching it would be inventing a staleness
//! problem to solve in a tree that changes while you work.
//!
//! A line scan rather than tree-sitter, because this runs over every file in the
//! project and needs exactly one fact from each. `defmodule` is a keyword at the
//! start of a line in every formatted Elixir file there is.

use serde::{Deserialize, Serialize};

/// One module, and where it is defined.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexedModule {
    /// The full name, nesting resolved: `MyApp.Accounts.User`.
    pub module: String,
    /// Absolute path, for opening.
    pub path: String,
    /// Relative to the project root, for display.
    pub rel: String,
    /// Defined under `test/` or in a `_test.exs` — a suite, not a module you
    /// navigate the app by.
    pub test: bool,
}

/// Every module a source file defines, with nested modules qualified by the
/// module around them — `defmodule User` inside `defmodule MyApp.Accounts` is
/// `MyApp.Accounts.User`, which is what Elixir itself calls it.
///
/// Nesting is tracked by indentation: a `defmodule` opens a scope at its column,
/// and the `end` at that same column closes it. That is exact for anything
/// `mix format` has touched, and a misjudged scope costs one wrong name in a
/// list, never a wrong answer about code you are reading — the outline of an
/// opened file comes from the real parser.
pub fn module_names(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    // (indent, full name) of every module still open.
    let mut open: Vec<(usize, String)> = Vec::new();

    for line in src.lines() {
        let indent = line.len() - line.trim_start().len();
        let text = line.trim_start();

        if let Some(rest) = text.strip_prefix("defmodule ") {
            let name: String = rest
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '_' || *c == '.')
                .collect();
            if name.is_empty() || !name.starts_with(|c: char| c.is_ascii_uppercase()) {
                continue;
            }
            // Anything open at this column or deeper has ended without us seeing
            // its `end` — a one-line `defmodule X, do: …`, most likely.
            open.retain(|(i, _)| *i < indent);
            let full = match open.last() {
                Some((_, outer)) => format!("{outer}.{name}"),
                None => name,
            };
            out.push(full.clone());
            // `defmodule X, do: ...` opens nothing.
            if !rest.contains(", do:") {
                open.push((indent, full));
            }
            continue;
        }

        if text == "end" || text.starts_with("end ") {
            if let Some(pos) = open.iter().rposition(|(i, _)| *i == indent) {
                open.truncate(pos);
            }
        }
    }
    out
}

/// Is this a test file? `test/` anywhere in the relative path, or the suffix
/// ExUnit itself requires.
pub fn is_test(rel: &str) -> bool {
    rel.ends_with("_test.exs") || rel.starts_with("test/") || rel.contains("/test/")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_single_module() {
        assert_eq!(
            module_names("defmodule MyApp.Billing do\n  def charge(a), do: a\nend\n"),
            ["MyApp.Billing"]
        );
    }

    #[test]
    fn a_nested_module_is_qualified_by_its_parent() {
        let src = "defmodule MyApp.Accounts do\n  defmodule User do\n    defstruct [:id]\n  end\n\n  def get(id), do: id\nend\n";
        assert_eq!(module_names(src), ["MyApp.Accounts", "MyApp.Accounts.User"]);
    }

    /// The `end` of a nested module must close only that module, or the sibling
    /// after it would be qualified by the wrong parent.
    #[test]
    fn siblings_after_a_nested_module_keep_the_right_parent() {
        let src = "defmodule A do\n  defmodule B do\n  end\n\n  defmodule C do\n    defmodule D do\n    end\n  end\nend\n\ndefmodule E do\nend\n";
        assert_eq!(module_names(src), ["A", "A.B", "A.C", "A.C.D", "E"]);
    }

    #[test]
    fn a_one_line_module_opens_no_scope() {
        let src = "defmodule A do\n  defmodule Error, do: defexception [:message]\n  defmodule B do\n  end\nend\n";
        assert_eq!(module_names(src), ["A", "A.Error", "A.B"]);
    }

    /// A function body full of `end`s at deeper columns must not close the module.
    #[test]
    fn inner_ends_do_not_close_the_module() {
        let src = "defmodule A do\n  def f(x) do\n    case x do\n      _ -> x\n    end\n  end\n\n  defmodule B do\n  end\nend\n";
        assert_eq!(module_names(src), ["A", "A.B"]);
    }

    #[test]
    fn a_config_script_defines_nothing() {
        assert!(module_names("import Config\n\nconfig :my_app, key: 1\n").is_empty());
    }

    #[test]
    fn tests_are_recognised_by_where_they_live() {
        assert!(is_test("test/my_app/accounts_test.exs"));
        assert!(is_test("apps/core/test/support/case.ex"));
        assert!(!is_test("lib/my_app/accounts.ex"));
        assert!(!is_test("lib/my_app/testing.ex"));
    }
}
