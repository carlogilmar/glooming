//! Merging a doc with re-parsed source.
//!
//! The contract, in one sentence: **your prose is never discarded.** Functions
//! that appeared get appended with empty slots; functions that vanished are
//! struck through and keep their explanation; everything else keeps what you
//! wrote, matched on `name/arity`.
//!
//! A rename reads as a delete plus an add — the old prose survives, struck
//! through, rather than silently moving to a function it wasn't written about.

use crate::parse::{Outline, Visibility};
use crate::seed::BLOCK_TAG;

/// One `- name/arity : prose` row of a block.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    /// As displayed: `create_user/1`, `search/1..2`, or `~~gone/0~~`.
    pub sig: String,
    pub prose: String,
}

impl Entry {
    /// The identity used for matching: `name/arity`, ignoring strikethrough and
    /// any `1..2` default-argument range (the top arity wins).
    fn key(&self) -> String {
        let bare = self.sig.trim_matches('~');
        match bare.split_once('/') {
            Some((name, arity)) => {
                let top = arity.rsplit("..").next().unwrap_or(arity);
                format!("{name}/{top}")
            }
            None => bare.to_string(),
        }
    }

    fn is_removed(&self) -> bool {
        self.sig.starts_with("~~")
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Block {
    pub info: String,
    pub public: Vec<Entry>,
    pub private: Vec<Entry>,
}

/// Rewrite the first `lgtm:functions` block in `markdown` against `outline`.
/// Markdown outside the block — every heading, paragraph and note you wrote —
/// is passed through untouched.
pub fn reconcile_markdown(markdown: &str, outline: &Outline) -> String {
    let Some((start, end)) = find_block(markdown) else {
        return markdown.to_string();
    };
    let existing = parse_block(&markdown[start..end]);
    let merged = merge(&existing, outline);

    let mut out = String::with_capacity(markdown.len());
    out.push_str(&markdown[..start]);
    out.push_str(&render_block(&merged));
    out.push_str(&markdown[end..]);
    out
}

/// Byte range of the whole fenced block, closing fence included.
fn find_block(md: &str) -> Option<(usize, usize)> {
    let open = md.find(&format!("```{BLOCK_TAG}"))?;
    let after = open + 3;
    let close = md[after..].find("```")? + after;
    // Include the closing fence and its newline.
    let end = md[close..]
        .find('\n')
        .map(|n| close + n + 1)
        .unwrap_or(md.len());
    Some((open, end))
}

pub fn parse_block(text: &str) -> Block {
    let mut block = Block::default();
    let mut group = Visibility::Public;

    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if i == 0 {
            block.info = trimmed.trim_start_matches('`').to_string();
            continue;
        }
        if trimmed.starts_with("```") {
            break;
        }
        match trimmed {
            "public:" => group = Visibility::Public,
            "private:" => group = Visibility::Private,
            _ => {
                if let Some(entry) = parse_row(trimmed) {
                    match group {
                        Visibility::Public => block.public.push(entry),
                        Visibility::Private => block.private.push(entry),
                    }
                }
            }
        }
    }
    block
}

/// `- create_user/1 : Entry point.` — everything after the first ` : ` is
/// prose, so explanations may contain colons freely.
fn parse_row(line: &str) -> Option<Entry> {
    let rest = line.strip_prefix("- ").or_else(|| line.strip_prefix("-"))?;
    let (sig, prose) = match rest.split_once(':') {
        Some((s, p)) => (s.trim(), p.trim()),
        None => (rest.trim(), ""),
    };
    if sig.is_empty() {
        return None;
    }
    Some(Entry {
        sig: sig.to_string(),
        prose: prose.to_string(),
    })
}

fn merge(existing: &Block, outline: &Outline) -> Block {
    let Some(module) = outline.modules.first() else {
        return existing.clone();
    };

    let mut out = Block {
        info: format!("{BLOCK_TAG} module={}", module.name),
        ..Default::default()
    };

    for visibility in [Visibility::Public, Visibility::Private] {
        let old = match visibility {
            Visibility::Public => &existing.public,
            Visibility::Private => &existing.private,
        };
        let mut merged = Vec::new();
        let mut matched = Vec::new();

        // Current functions, keeping any prose already written for them —
        // including prose that was under the *other* visibility, so flipping
        // def↔defp doesn't lose the explanation. Alphabetical, matching the
        // seeder, so reconciling never silently reshuffles the table.
        let mut current: Vec<_> = module
            .functions
            .iter()
            .filter(|f| f.visibility == visibility)
            .collect();
        current.sort_by(|a, b| a.name.cmp(&b.name).then(a.arity.cmp(&b.arity)));

        for f in current {
            let sig = if f.min_arity < f.arity {
                format!("{}/{}..{}", f.name, f.min_arity, f.arity)
            } else {
                format!("{}/{}", f.name, f.arity)
            };
            let key = format!("{}/{}", f.name, f.arity);
            let prose = existing
                .public
                .iter()
                .chain(existing.private.iter())
                .find(|e| e.key() == key && !e.is_removed())
                .map(|e| e.prose.clone())
                .unwrap_or_default();
            matched.push(key);
            merged.push(Entry { sig, prose });
        }

        // Anything that used to be here and isn't any more: struck through,
        // prose intact. Already-struck rows stay struck rather than doubling up.
        for e in old {
            if matched.contains(&e.key()) {
                continue;
            }
            if e.prose.is_empty() && e.is_removed() {
                continue; // a struck row nobody ever explained: let it go
            }
            let sig = if e.is_removed() {
                e.sig.clone()
            } else {
                format!("~~{}~~", e.sig)
            };
            merged.push(Entry {
                sig,
                prose: e.prose.clone(),
            });
        }

        match visibility {
            Visibility::Public => out.public = merged,
            Visibility::Private => out.private = merged,
        }
    }
    out
}

pub fn render_block(block: &Block) -> String {
    let mut out = format!("```{}\n", block.info);
    for (label, entries) in [("public:", &block.public), ("private:", &block.private)] {
        if entries.is_empty() {
            continue;
        }
        out.push_str(label);
        out.push('\n');
        let width = entries.iter().map(|e| e.sig.len()).max().unwrap_or(0);
        for e in entries {
            let sig = &e.sig;
            out.push_str(&format!("  - {sig:width$} : {}\n", e.prose));
        }
    }
    out.push_str("```\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::elixir;

    const DOC: &str = r#"# MyApp.Accounts

> The only module allowed to touch users.

## Surface

```lgtm:functions module=MyApp.Accounts
public:
  - create_user/1 : Entry point. Validates, then inserts.
  - get_user/1    : Plain fetch by id.
private:
  - normalize/1   : Trims and downcases the email.
```

## Notes

Any new validation belongs in the changeset.
"#;

    fn reconcile_with(src: &str) -> String {
        reconcile_markdown(DOC, &elixir::parse(src).unwrap())
    }

    #[test]
    fn unchanged_code_keeps_every_explanation() {
        let out = reconcile_with(
            "defmodule MyApp.Accounts do
  def create_user(a), do: a
  def get_user(id), do: id
  defp normalize(a), do: a
end",
        );
        assert!(out.contains("Entry point. Validates, then inserts."));
        assert!(out.contains("Plain fetch by id."));
        assert!(out.contains("Trims and downcases the email."));
        assert!(!out.contains("~~"), "nothing removed:\n{out}");
    }

    #[test]
    fn a_new_function_is_appended_with_an_empty_slot() {
        let out = reconcile_with(
            "defmodule MyApp.Accounts do
  def create_user(a), do: a
  def get_user(id), do: id
  def delete_user(id), do: id
  defp normalize(a), do: a
end",
        );
        assert!(out.contains("delete_user/1"));
        let row = out
            .lines()
            .find(|l| l.contains("delete_user/1"))
            .unwrap()
            .trim_end();
        assert!(row.ends_with(':'), "empty slot, got: {row}");
        assert!(out.contains("Entry point. Validates, then inserts."));
    }

    #[test]
    fn a_removed_function_is_struck_through_but_keeps_its_prose() {
        let out = reconcile_with(
            "defmodule MyApp.Accounts do
  def create_user(a), do: a
  defp normalize(a), do: a
end",
        );
        assert!(out.contains("~~get_user/1~~"), "struck:\n{out}");
        assert!(
            out.contains("Plain fetch by id."),
            "prose survives removal:\n{out}"
        );
    }

    #[test]
    fn prose_outside_the_block_is_untouched() {
        let out = reconcile_with("defmodule MyApp.Accounts do\n  def create_user(a), do: a\nend");
        assert!(out.contains("> The only module allowed to touch users."));
        assert!(out.contains("Any new validation belongs in the changeset."));
        assert!(out.contains("## Notes"));
    }

    #[test]
    fn changing_visibility_carries_the_explanation_across() {
        // normalize/1 becomes public.
        let out = reconcile_with(
            "defmodule MyApp.Accounts do
  def create_user(a), do: a
  def get_user(id), do: id
  def normalize(a), do: a
end",
        );
        let public_part = &out[out.find("public:").unwrap()..out.find("```\n\n## Notes").unwrap()];
        assert!(
            public_part.contains("Trims and downcases the email."),
            "prose moved with the function:\n{out}"
        );
    }

    #[test]
    fn an_arity_change_reads_as_remove_plus_add() {
        let out = reconcile_with(
            "defmodule MyApp.Accounts do
  def create_user(a, b), do: {a, b}
  def get_user(id), do: id
  defp normalize(a), do: a
end",
        );
        assert!(out.contains("create_user/2"), "new arity added:\n{out}");
        assert!(out.contains("~~create_user/1~~"), "old arity struck:\n{out}");
        assert!(out.contains("Entry point. Validates, then inserts."));
    }

    #[test]
    fn a_doc_without_a_block_passes_through_verbatim() {
        let md = "# Notes\n\nJust prose, no block.\n";
        let outline = elixir::parse("defmodule A do\n def x, do: 1\nend").unwrap();
        assert_eq!(reconcile_markdown(md, &outline), md);
    }

    #[test]
    fn rows_survive_a_parse_render_round_trip() {
        let block = parse_block(
            "```lgtm:functions module=A\npublic:\n  - f/1 : does a thing: with a colon\n```",
        );
        assert_eq!(block.public.len(), 1);
        assert_eq!(block.public[0].prose, "does a thing: with a colon");
        assert!(render_block(&block).contains("does a thing: with a colon"));
    }

    #[test]
    fn default_argument_ranges_match_their_top_arity() {
        let e = Entry {
            sig: "search/1..2".into(),
            prose: "p".into(),
        };
        assert_eq!(e.key(), "search/2");
    }
}
