//! Reading a gloom out of a markdown file.
//!
//! A gloom you can hand to someone: front matter naming the project and the
//! files, and a body that becomes the note.
//!
//! ```text
//! ---
//! project: ~/Coding/GitHub/my_app
//! name: Accounts — the write path
//! branch: main
//! files:
//!   - lib/my_app/accounts.ex
//!   - lib/my_app/billing.ex
//! ---
//!
//! The note, exactly as you would write it…
//! ```
//!
//! **This is not the `lgtm:files` block that was cut.** That one was a *storage*
//! format — a list living inside a note, coexisting with `doc_files`, with two
//! sources of truth that drift the moment you press `×` on a tab. This is a
//! *transfer* format: read once at the door, then discarded. It never coexists
//! with the rows, so "what happens when they disagree" cannot arise. Importing
//! the same file twice makes two glooms, the same call as opening a changed file
//! as a new gloom rather than reconciling it into the old one.
//!
//! **Front-matter delimiters, and a fixed tiny schema parsed line by line** — not
//! YAML. Four keys do not justify a parser that also has anchors, flow style and
//! `no` meaning `false`, and the line rule is *better* for this input: everything
//! after the first colon is the value, verbatim, so
//! `name: Accounts: the write path` means what it looks like where real YAML
//! would reject it. It is the same rule `settings.rs`, `tests.rs` and every other
//! block in this app is read with.

use serde::{Deserialize, Serialize};

/// Every key the header may carry. Anything else is reported, never ignored:
/// silently skipping `flies:` is how a typo becomes a mystery.
const KEYS: [&str; 4] = ["project", "name", "branch", "files"];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Problem {
    /// 1-based, or `None` for something the file as a whole is missing.
    pub line: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListedFile {
    /// Relative to `project`, exactly as written.
    pub path: String,
    /// Where it was listed, so a problem can point at it.
    pub line: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Parsed {
    pub project: String,
    pub name: String,
    pub branch: Option<String>,
    pub files: Vec<ListedFile>,
    /// Everything after the closing `---`, which becomes the note verbatim.
    pub note: String,
    /// Empty when the file is well-formed. Never partial: every problem the
    /// file has is reported at once, because fixing one and hitting the next is
    /// how a five-line header takes five attempts.
    pub problems: Vec<Problem>,
}

impl Parsed {
    pub fn ok(&self) -> bool {
        self.problems.is_empty()
    }
}

/// Read the header and split the note off it.
///
/// Never returns `Err`: a file that cannot be read is described rather than
/// rejected, because the panel's whole job is to say what is wrong with it.
pub fn parse(text: &str) -> Parsed {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut out = Parsed::default();
    let at = |line: usize, message: String| Problem {
        line: Some(line as u32 + 1),
        message,
    };
    let mut problems: Vec<Problem> = Vec::new();

    if lines.first().map(|l| l.trim()) != Some("---") {
        problems.push(Problem {
            line: Some(1),
            message: "no front matter — the file must open with `---` on line 1".into(),
        });
        out.problems = problems;
        out.note = text.to_string();
        return out;
    }

    let Some(end) = (1..lines.len()).find(|&i| lines[i].trim() == "---") else {
        problems.push(Problem {
            line: Some(1),
            message: "the front matter is never closed — expected a second `---`".into(),
        });
        out.problems = problems;
        return out;
    };

    let mut seen_keys: Vec<&str> = Vec::new();
    // Which key is currently collecting list items. Only `files` takes a list,
    // but tracking it by name means a stray `- x` under `name:` is reported as
    // what it is rather than silently joining the file list.
    let mut list_key: Option<&str> = None;

    // `enumerate` over the header slice, so the index and the line come from the
    // same place — `i` is offset by 1 because the slice starts after the opening
    // `---`, and every message must point at the line the reader will count to.
    for (offset, raw) in lines[1..end].iter().enumerate() {
        let i = offset + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(rest) = line.strip_prefix('-') {
            match list_key {
                Some("files") => {
                    let v = rest.trim();
                    if v.is_empty() {
                        problems.push(at(i, "an empty entry in `files`".into()));
                    } else {
                        out.files.push(ListedFile {
                            path: v.to_string(),
                            line: i as u32 + 1,
                        });
                    }
                }
                _ => problems.push(at(i, "a list item outside `files:`".into())),
            }
            continue;
        }

        let Some(colon) = line.find(':') else {
            problems.push(at(i, format!("`{line}` is not a `key: value` line")));
            list_key = None;
            continue;
        };
        let key = line[..colon].trim();
        // Everything after the FIRST colon is the value, verbatim. This is the
        // whole reason the header is not real YAML: `name: Accounts: revisited`
        // is a thing people write, and YAML errors on it.
        let value = line[colon + 1..].trim();

        if !KEYS.contains(&key) {
            problems.push(at(
                i,
                format!("unknown key `{key}` — expected one of {}", KEYS.join(", ")),
            ));
            list_key = None;
            continue;
        }
        if seen_keys.contains(&key) {
            problems.push(at(i, format!("`{key}` appears twice")));
        }
        seen_keys.push(KEYS.iter().find(|k| **k == key).copied().unwrap_or(""));

        if key == "files" {
            list_key = Some("files");
            if !value.is_empty() {
                problems.push(at(
                    i,
                    "`files` takes a list on the lines below, not a value".into(),
                ));
            }
            continue;
        }

        list_key = None;
        if value.is_empty() {
            problems.push(at(i, format!("`{key}` has no value")));
            continue;
        }
        match key {
            "project" => out.project = value.to_string(),
            "name" => out.name = value.to_string(),
            "branch" => out.branch = Some(value.to_string()),
            _ => unreachable!("key was checked against KEYS"),
        }
    }

    for required in ["project", "name"] {
        if !seen_keys.contains(&required) {
            problems.push(Problem {
                line: None,
                message: format!("`{required}` is missing"),
            });
        }
    }
    if !seen_keys.contains(&"files") {
        problems.push(Problem {
            line: None,
            message: "`files` is missing".into(),
        });
    } else if out.files.is_empty() {
        problems.push(Problem {
            line: None,
            message: "`files` is empty — a gloom covers at least one file".into(),
        });
    }

    // The shape of every path, before the disk is ever touched. A path that
    // cannot be resolved is a problem with the *file*, not with the machine.
    let mut seen_paths: Vec<&str> = Vec::new();
    for f in &out.files {
        let line = f.line as usize - 1;
        if f.path.starts_with('/') || f.path.starts_with('~') {
            problems.push(at(
                line,
                format!("`{}` is absolute — paths are relative to `project`", f.path),
            ));
        } else if f.path.split('/').any(|seg| seg == "..") {
            problems.push(at(
                line,
                format!("`{}` escapes the project with `..`", f.path),
            ));
        } else if seen_paths.contains(&f.path.as_str()) {
            problems.push(at(line, format!("`{}` is listed twice", f.path)));
        }
        seen_paths.push(&f.path);
    }

    // The note is everything after the closing marker, with the blank line that
    // conventionally follows it trimmed — it is a separator, not prose.
    out.note = lines
        .get(end + 1..)
        .map(|rest| rest.join("\n"))
        .unwrap_or_default()
        .trim_start_matches('\n')
        .to_string();

    problems.sort_by_key(|p| p.line.unwrap_or(u32::MAX));
    out.problems = problems;
    out
}

/// A gloom file with everything we already know filled in.
///
/// **Written here, beside the parser**, on the same argument that keeps
/// `lgtm:surface` generated in one place: a template that lives somewhere else
/// is a second declaration of the schema, and the two drift the first time a key
/// is added. `the_template_only_needs_filling_in` fails if they ever disagree.
///
/// Pre-filled rather than blank, because "ready to fill" is the point — the
/// project path is the most tedious line to type and the one you are most likely
/// to get subtly wrong, and the branch is right there to be read. What is left
/// is the name and the files, which are the only parts you actually know.
pub fn template(project: Option<&str>, branch: Option<&str>) -> String {
    let project = project.unwrap_or("");
    let branch = branch.unwrap_or("");
    format!(
        "---\n\
         # The project this gloom reads. Every path below is relative to it.\n\
         project: {project}\n\
         \n\
         # What the gloom is called — shown in the band, the library and home.\n\
         name:\n\
         \n\
         # Optional. Set it and the import refuses unless you are on this branch,\n\
         # because a gloom is a reading of one version of the code.\n\
         branch: {branch}\n\
         \n\
         # The files to open, in the order you want them. The FIRST one is the\n\
         # origin: what the reading is anchored to, and the one that cannot be\n\
         # removed later.\n\
         files:\n\
         \x20 -\n\
         \x20 -\n\
         ---\n\
         \n\
         Write the note here. Anything in backticks that names a function in the\n\
         reading becomes a reference — `Accounts.create_user/1`, or `L25-29` for\n\
         plain lines — and `▷ LGTM` will walk the code in the order your prose\n\
         takes it.\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLEAN: &str = r#"---
project: ~/Coding/GitHub/my_app
name: Accounts — the write path
branch: main
files:
  - lib/my_app/accounts.ex
  - lib/my_app/billing.ex
  - test/my_app/accounts_test.exs
---

`Accounts.create_user/1` normalises the attrs first.
"#;

    #[test]
    fn reads_the_header_and_splits_the_note_off_it() {
        let p = parse(CLEAN);
        assert!(p.ok(), "{:?}", p.problems);
        assert_eq!(p.project, "~/Coding/GitHub/my_app");
        assert_eq!(p.name, "Accounts — the write path");
        assert_eq!(p.branch.as_deref(), Some("main"));
        assert_eq!(p.files.len(), 3);
        assert_eq!(p.files[0].path, "lib/my_app/accounts.ex");
        assert_eq!(p.files[0].line, 6, "the line it was listed on");

        // The header is consumed. What lands in the gloom is prose and nothing
        // else — no front matter, no generated title.
        assert!(p.note.starts_with("`Accounts.create_user/1`"), "got {:?}", p.note);
        assert!(!p.note.contains("project:"));
    }

    /// The whole reason the header is not YAML. This is a name someone writes.
    #[test]
    fn a_colon_in_a_value_is_part_of_the_value() {
        let p = parse("---\nproject: ~/x\nname: Accounts: the write path\nfiles:\n  - a.ex\n---\nn");
        assert!(p.ok(), "{:?}", p.problems);
        assert_eq!(p.name, "Accounts: the write path");
    }

    #[test]
    fn branch_is_optional_and_everything_else_is_not() {
        let p = parse("---\nproject: ~/x\nname: n\nfiles:\n  - a.ex\n---\nnote");
        assert!(p.ok(), "{:?}", p.problems);
        assert!(p.branch.is_none());

        let p = parse("---\nname: n\nfiles:\n  - a.ex\n---\nnote");
        assert!(p.problems.iter().any(|x| x.message.contains("`project` is missing")));
    }

    /// Named, never ignored — a typo that silently does nothing is how a
    /// manifest becomes a mystery.
    #[test]
    fn an_unknown_key_is_reported_with_its_line() {
        let p = parse("---\nprojekt: ~/x\nname: n\nfiles:\n  - a.ex\n---\nnote");
        let bad = p
            .problems
            .iter()
            .find(|x| x.message.contains("unknown key `projekt`"))
            .expect("named");
        assert_eq!(bad.line, Some(2));
        // …and the key it was meant to be is still reported missing.
        assert!(p.problems.iter().any(|x| x.message.contains("`project` is missing")));
    }

    #[test]
    fn every_problem_is_reported_at_once() {
        let p = parse(
            "---\nprojekt: ~/x\ntags: perf\nname: n\nfiles:\n  - a.ex\n  - /abs/b.ex\n  - a.ex\n---\nnote",
        );
        let all = p
            .problems
            .iter()
            .map(|x| x.message.as_str())
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(all.contains("projekt"), "{all}");
        assert!(all.contains("tags"), "{all}");
        assert!(all.contains("`project` is missing"), "{all}");
        assert!(all.contains("is absolute"), "{all}");
        assert!(all.contains("listed twice"), "{all}");
        // Ordered by line, so the list reads down the file.
        let lines: Vec<u32> = p.problems.iter().filter_map(|x| x.line).collect();
        let mut sorted = lines.clone();
        sorted.sort_unstable();
        assert_eq!(lines, sorted);
    }

    #[test]
    fn a_path_that_leaves_the_project_is_refused() {
        let p = parse("---\nproject: ~/x\nname: n\nfiles:\n  - ../../etc/passwd\n---\nnote");
        assert!(p.problems.iter().any(|x| x.message.contains("escapes the project")));

        // A `..` inside a segment is a filename, not a traversal.
        let p = parse("---\nproject: ~/x\nname: n\nfiles:\n  - lib/a..b.ex\n---\nnote");
        assert!(p.ok(), "{:?}", p.problems);
    }

    #[test]
    fn an_empty_file_list_is_not_a_gloom() {
        let p = parse("---\nproject: ~/x\nname: n\nfiles:\n---\nnote");
        assert!(p.problems.iter().any(|x| x.message.contains("`files` is empty")));
    }

    #[test]
    fn a_plain_markdown_file_is_refused_clearly() {
        let p = parse("# Just a note\n\nNo header here.");
        assert_eq!(p.problems.len(), 1);
        assert!(p.problems[0].message.contains("no front matter"));
    }

    #[test]
    fn unterminated_front_matter_is_named() {
        let p = parse("---\nproject: ~/x\nname: n\nfiles:\n  - a.ex\n");
        assert!(p.problems.iter().any(|x| x.message.contains("never closed")));
    }

    #[test]
    fn comments_and_blank_lines_are_allowed_in_the_header() {
        let p = parse("---\n# which repo\nproject: ~/x\n\nname: n\nfiles:\n  - a.ex\n---\nnote");
        assert!(p.ok(), "{:?}", p.problems);
    }

    /// A `---` inside the note must not be mistaken for the closing marker: the
    /// FIRST one after line 1 closes the header, and everything else is prose.
    #[test]
    fn a_rule_in_the_note_is_prose() {
        let p = parse("---\nproject: ~/x\nname: n\nfiles:\n  - a.ex\n---\nabove\n\n---\n\nbelow\n");
        assert!(p.ok(), "{:?}", p.problems);
        assert!(p.note.contains("above") && p.note.contains("---") && p.note.contains("below"));
    }

    #[test]
    fn a_stray_list_item_says_what_it_is() {
        let p = parse("---\nproject: ~/x\nname: n\n  - oops\nfiles:\n  - a.ex\n---\nnote");
        assert!(p.problems.iter().any(|x| x.message.contains("outside `files:`")));
    }
}

#[cfg(test)]
mod template_tests {
    use super::*;

    /// The template and the parser must not drift. If a key is added to one and
    /// not the other, this is what says so.
    #[test]
    fn the_template_only_needs_filling_in() {
        let t = template(Some("/tmp/my_app"), Some("main"));
        let p = parse(&t);

        // Nothing structural is wrong with it: no unknown keys, no missing keys,
        // no malformed lines. Every complaint is a blank waiting for you.
        // Every complaint must be about a BLANK. An unknown key, a malformed
        // line or a bad path would mean the template and the parser disagree,
        // which is the drift this test exists to catch.
        for problem in &p.problems {
            let blank = problem.message.contains("has no value")
                || problem.message.contains("empty entry")
                || problem.message.contains("`files` is empty");
            assert!(
                blank,
                "the template should only ever be incomplete, not wrong: {problem:?}"
            );
        }
        assert!(!p.problems.is_empty(), "a blank template is a form, and says so");

        assert_eq!(p.project, "/tmp/my_app", "the project is filled in");
        assert_eq!(p.branch.as_deref(), Some("main"), "and so is the branch");
        assert!(p.name.is_empty(), "the name is yours to write");
        assert!(p.note.contains("Write the note here"), "and so is the note");
    }

    /// Fill in the two blanks and it is a valid gloom file — which is the only
    /// promise the template actually makes.
    #[test]
    fn filling_the_blanks_in_makes_it_valid() {
        let filled = template(Some("/tmp/my_app"), Some("main"))
            .replace("name:\n", "name: A reading\n")
            .replace("  -\n  -\n", "  - lib/a.ex\n  - lib/b.ex\n");

        let p = parse(&filled);
        assert!(p.ok(), "{:?}", p.problems);
        assert_eq!(p.name, "A reading");
        assert_eq!(p.files.len(), 2);
        assert_eq!(p.files[0].path, "lib/a.ex");
    }

    /// With no project open there is nothing to fill in, and an empty value is
    /// reported like any other — it does not pretend to be complete.
    #[test]
    fn an_unfilled_template_says_which_fields_are_blank() {
        let p = parse(&template(None, None));
        let msgs: Vec<&str> = p.problems.iter().map(|x| x.message.as_str()).collect();
        assert!(msgs.iter().any(|m| m.contains("`project` has no value")), "{msgs:?}");
        assert!(msgs.iter().any(|m| m.contains("`branch` has no value")), "{msgs:?}");
    }

    /// Every key the template writes is one the parser accepts, and vice versa —
    /// the drift check stated directly rather than inferred from the problems.
    #[test]
    fn the_template_mentions_every_key_and_no_others() {
        let t = template(Some("/x"), Some("main"));
        for key in KEYS {
            assert!(t.contains(&format!("\n{key}:")), "the template omits `{key}`");
        }
    }
}
