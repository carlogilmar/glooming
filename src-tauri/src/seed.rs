//! Turning an [`Outline`] into the starter markdown doc.
//!
//! The seed is deliberately mostly-empty: it lays out every function with a
//! blank explanation so the gaps are visible. Those gaps are the nudge — an
//! unexplained function renders as a ghost "explain…" placeholder.

use crate::parse::{ModuleInfo, Outline, Visibility};

/// The fence tag the frontend renderer looks for.
pub const BLOCK_TAG: &str = "lgtm:functions";

pub fn seed_markdown(outline: &Outline) -> String {
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

    out.push_str("## Surface\n\n");
    out.push_str(&functions_block(module));
    out.push_str("\n## Notes\n\n");
    out
}

/// The ```lgtm:functions block for one module.
pub fn functions_block(module: &ModuleInfo) -> String {
    let mut out = format!("```{BLOCK_TAG} module={}\n", module.name);

    for visibility in [Visibility::Public, Visibility::Private] {
        let group: Vec<_> = module
            .functions
            .iter()
            .filter(|f| f.visibility == visibility)
            .collect();
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
        for f in group {
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

    fn seeded() -> String {
        seed_markdown(&elixir::parse(SAMPLE).unwrap())
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
        let line = md
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
        assert_eq!(md.matches("```").count(), 2, "opened and closed once");
    }

    #[test]
    fn a_file_with_no_module_still_seeds_something() {
        let outline = elixir::parse("x = 1\n").unwrap();
        assert!(seed_markdown(&outline).starts_with("# Untitled"));
    }
}
