//! A Broadway module, read for its view: the shape of `start_link`'s options and
//! the `handle_batch` clauses. `mockup/broadway.html` is the contract.
//!
//! Everything is read statically, from the syntax tree. The options are taken
//! only when they are a literal keyword list passed to `Broadway.start_link/2`;
//! built anywhere else, they are reported as runtime rather than guessed at. A
//! value that is neither a literal nor an app-env read is `Runtime`, carried as
//! its source text — the view names it by what computes it.
//!
//! Shapes, confirmed by dumping the tree of `tests/fixtures/orders_pipeline.ex`:
//!
//! ```text
//! use Broadway                      call(use, args[ alias(Broadway) ])
//! Broadway.start_link(M, opts…)     call(dot[alias, identifier], args[ M, keywords[pair…] ])
//! producer: [module: {P, o}, …]     pair(keyword, list[ keywords[ pair(module, tuple[alias, keywords]) ] ])
//! context: %{a: 1}                  pair(keyword, map[ map_content[ keywords[ pair… ] ] ])
//! def handle_batch(:default, …)     call(def, args[ call(handle_batch, args[ atom, … ]) ])
//! ```

use super::{
    BatchClause, BroadwayContext, BroadwayInfo, BroadwayOptions, BroadwayProducer, BroadwayStage,
    ContextEntry, Knob, KnobValue, Range,
};
use tree_sitter::Node;

fn text<'a>(node: Node, src: &'a str) -> &'a str {
    &src[node.byte_range()]
}

fn child_of_kind<'a>(node: Node<'a>, kind: &str) -> Option<Node<'a>> {
    let mut cursor = node.walk();
    let found = node.named_children(&mut cursor).find(|c| c.kind() == kind);
    found
}

/// A node's named children, comments left out. A comment is a named node in this
/// grammar, so `{BroadwaySQS.Producer, # why…\n queue_url: …}` has THREE children,
/// and taking the second as the options read the comment — the real Weather
/// pipeline's producer came out with no options at all.
fn named_children<'a>(node: Node<'a>) -> Vec<Node<'a>> {
    let mut cursor = node.walk();
    let found: Vec<Node> = node.named_children(&mut cursor).filter(|c| c.kind() != "comment").collect();
    found
}

/// `use`, `def` — an identifier call's name.
fn ident_call(node: Node, src: &str) -> Option<String> {
    if node.kind() != "call" {
        return None;
    }
    let t = node.named_child(0)?;
    (t.kind() == "identifier").then(|| text(t, src).to_string())
}

/// `Broadway.start_link` — a remote call's `Module.fun`.
fn remote_call(node: Node, src: &str) -> Option<(String, String)> {
    if node.kind() != "call" {
        return None;
    }
    let dot = node.named_child(0)?;
    if dot.kind() != "dot" {
        return None;
    }
    let m = dot.named_child(0)?;
    let f = dot.named_child(1)?;
    Some((text(m, src).to_string(), text(f, src).to_string()))
}

/// Source text on one line, trimmed for a label.
fn compact(s: &str) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if one.chars().count() > 80 {
        format!("{}…", one.chars().take(79).collect::<String>())
    } else {
        one
    }
}

/// A keyword list's pairs, from a `list` or a bare `keywords` node.
fn pairs<'a>(node: Node<'a>) -> Vec<Node<'a>> {
    let kw = if node.kind() == "keywords" {
        Some(node)
    } else {
        child_of_kind(node, "keywords")
    };
    kw.map(|k| named_children(k).into_iter().filter(|c| c.kind() == "pair").collect())
        .unwrap_or_default()
}

/// `name: ` → `name`.
fn key(pair: Node, src: &str) -> String {
    pair.named_child(0)
        .map(|k| text(k, src).trim().trim_end_matches(':').to_string())
        .unwrap_or_default()
}

fn value(pair: Node) -> Option<Node> {
    pair.named_child(1)
}

/// Is this module a Broadway pipeline? `use Broadway` in its body.
pub fn uses_broadway(body: Node, src: &str) -> bool {
    named_children(body).into_iter().any(|c| {
        ident_call(c, src).as_deref() == Some("use")
            && c.named_child(1)
                .and_then(|a| a.named_child(0))
                .is_some_and(|m| text(m, src) == "Broadway")
    })
}

/// Read a Broadway module's view. `module` resolves `__MODULE__`.
pub fn broadway_info(module: &str, body: Node, src: &str) -> BroadwayInfo {
    let call = find_start_link(body, src);
    let (options, runtime_options) = match call.and_then(|c| c.named_child(1)) {
        Some(args) => {
            // Positional args: the module, then the options. A trailing keyword
            // list is a `keywords` node; anything else is built at runtime.
            let rest: Vec<Node> = named_children(args).into_iter().skip(1).collect();
            match rest.first() {
                Some(kw) if kw.kind() == "keywords" || kw.kind() == "list" => {
                    (Some(options(module, *kw, src)), None)
                }
                Some(expr) => (None, Some(compact(text(*expr, src)))),
                None => (None, None),
            }
        }
        None => (None, None),
    };
    BroadwayInfo {
        call: call.map(Range::of),
        options,
        runtime_options,
        batch_clauses: batch_clauses(body, src),
    }
}

/// The first `Broadway.start_link(...)` anywhere in the body.
fn find_start_link<'a>(node: Node<'a>, src: &str) -> Option<Node<'a>> {
    if remote_call(node, src).is_some_and(|(m, f)| m == "Broadway" && f == "start_link") {
        return Some(node);
    }
    named_children(node).into_iter().find_map(|c| find_start_link(c, src))
}

fn options(module: &str, kw: Node, src: &str) -> BroadwayOptions {
    let mut out = BroadwayOptions {
        range: Range::of(kw),
        name: None,
        producer: None,
        processors: Vec::new(),
        batchers: Vec::new(),
        context: None,
    };
    for pair in pairs(kw) {
        let Some(v) = value(pair) else { continue };
        match key(pair, src).as_str() {
            "name" => {
                out.name = Some(Knob { value: knob_value(module, v, src), range: Range::of(pair) })
            }
            "producer" => out.producer = Some(producer(module, pair, v, src)),
            "processors" => out.processors = stages(module, v, src),
            "batchers" => out.batchers = stages(module, v, src),
            "context" => out.context = Some(context(module, pair, v, src)),
            _ => {}
        }
    }
    out
}

fn producer(module: &str, pair: Node, list: Node, src: &str) -> BroadwayProducer {
    let mut p = BroadwayProducer {
        module: None,
        module_range: None,
        options: Vec::new(),
        options_range: None,
        concurrency: None,
        range: Range::of(pair),
    };
    for opt in pairs(list) {
        let Some(v) = value(opt) else { continue };
        match key(opt, src).as_str() {
            "module" => {
                p.module_range = Some(Range::of(opt));
                // `{BroadwaySQS.Producer, opts}` or a bare `MyProducer`.
                match v.kind() {
                    "tuple" => {
                        let parts = named_children(v);
                        p.module = parts.first().map(|m| text(*m, src).to_string());
                        if let Some(o) = parts.get(1).filter(|o| o.kind() == "keywords" || o.kind() == "list") {
                            p.options = pairs(*o).into_iter().map(|q| key(q, src)).collect();
                            p.options_range = Some(Range::of(*o));
                        }
                    }
                    "alias" => p.module = Some(text(v, src).to_string()),
                    _ => p.module = Some(compact(text(v, src))),
                }
            }
            "concurrency" => {
                p.concurrency = Some(Knob { value: knob_value(module, v, src), range: Range::of(opt) })
            }
            _ => {}
        }
    }
    p
}

/// `[default: [concurrency: 4, …], ledger: […]]` — one stage per name.
fn stages(module: &str, list: Node, src: &str) -> Vec<BroadwayStage> {
    pairs(list)
        .into_iter()
        .map(|pair| {
            let mut st = BroadwayStage {
                name: key(pair, src),
                range: Range::of(pair),
                concurrency: None,
                max_demand: None,
                batch_size: None,
                batch_timeout: None,
            };
            if let Some(opts) = value(pair) {
                for opt in pairs(opts) {
                    let Some(v) = value(opt) else { continue };
                    let k = Some(Knob { value: knob_value(module, v, src), range: Range::of(opt) });
                    match key(opt, src).as_str() {
                        "concurrency" => st.concurrency = k,
                        "max_demand" => st.max_demand = k,
                        "batch_size" => st.batch_size = k,
                        "batch_timeout" => st.batch_timeout = k,
                        _ => {}
                    }
                }
            }
            st
        })
        .collect()
}

fn context(module: &str, pair: Node, v: Node, src: &str) -> BroadwayContext {
    let entries: Vec<ContextEntry> = if v.kind() == "map" {
        child_of_kind(v, "map_content")
            .map(|c| {
                pairs(c)
                    .into_iter()
                    .filter_map(|p| {
                        Some(ContextEntry {
                            key: key(p, src),
                            value: knob_value(module, value(p)?, src),
                            range: Range::of(p),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    // A map we could not read entry by entry (`%{"a" => 1}`, `%{conf | x: 1}`)
    // is runtime too: the view says so rather than drawing an empty map.
    let runtime = (entries.is_empty() && !(v.kind() == "map" && text(v, src).trim() == "%{}"))
        .then(|| compact(text(v, src)));
    BroadwayContext { range: Range::of(pair), entries, runtime }
}

/// Where a value comes from.
fn knob_value(module: &str, v: Node, src: &str) -> KnobValue {
    match v.kind() {
        "integer" | "float" | "atom" | "boolean" | "nil" | "alias" => {
            KnobValue::Literal { text: text(v, src).to_string() }
        }
        "string" => KnobValue::Literal {
            text: format!(
                "\"{}\"",
                child_of_kind(v, "quoted_content").map(|q| text(q, src)).unwrap_or("")
            ),
        },
        // `__MODULE__` is the module it is written in, so the view can say which.
        "identifier" if text(v, src) == "__MODULE__" => KnobValue::Literal { text: module.to_string() },
        "call" => {
            if let Some((m, f)) = remote_call(v, src) {
                let args: Vec<Node> = v.named_child(1).map(named_children).unwrap_or_default();
                let required = f.ends_with('!');
                match (m.as_str(), f.as_str()) {
                    ("Application", "get_env" | "fetch_env!" | "fetch_env" | "compile_env" | "compile_env!") => {
                        return KnobValue::Env {
                            key: args.get(1).map(|k| text(*k, src).to_string()).unwrap_or_default(),
                            default: args.get(2).map(|d| compact(text(*d, src))),
                            required,
                        };
                    }
                    ("System", "get_env" | "fetch_env!" | "fetch_env") => {
                        return KnobValue::Env {
                            key: args.first().map(|k| text(*k, src).to_string()).unwrap_or_default(),
                            default: args.get(1).map(|d| compact(text(*d, src))),
                            required,
                        };
                    }
                    _ => {}
                }
            }
            KnobValue::Runtime { text: compact(text(v, src)) }
        }
        _ => KnobValue::Runtime { text: compact(text(v, src)) },
    }
}

/// Every `def handle_batch(...)` clause, with the batcher its first argument
/// names — or `None` for a variable, which answers for every batcher.
fn batch_clauses(body: Node, src: &str) -> Vec<BatchClause> {
    let mut out = Vec::new();
    for c in named_children(body) {
        if ident_call(c, src).as_deref() != Some("def") {
            continue;
        }
        let Some(head) = c.named_child(1).and_then(|a| a.named_child(0)) else { continue };
        // `def handle_batch(…) when guard` wraps the head in a binary operator.
        let head = if head.kind() == "binary_operator" { head.named_child(0).unwrap_or(head) } else { head };
        if ident_call(head, src).as_deref() != Some("handle_batch") {
            continue;
        }
        let first = head.named_child(1).and_then(|a| a.named_child(0));
        let batcher = first
            .filter(|n| n.kind() == "atom")
            .map(|n| text(n, src).trim_start_matches(':').to_string());
        out.push(BatchClause { batcher, range: Range::of(c) });
    }
    out
}

#[cfg(test)]
mod tests {
    use crate::parse::{elixir, Behaviour, BroadwayInfo, KnobValue};

    fn info(src: &str) -> BroadwayInfo {
        let o = elixir::parse(src).expect("parses");
        match o.modules[0].behaviour.clone() {
            Some(Behaviour::Broadway(b)) => b,
            None => panic!("not recognised as Broadway"),
        }
    }

    fn fixture() -> String {
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/orders_pipeline.ex"))
            .expect("fixture")
    }

    fn lit(t: &str) -> KnobValue {
        KnobValue::Literal { text: t.into() }
    }

    #[test]
    fn reads_the_wiring_of_a_complete_pipeline() {
        let b = info(&fixture());
        let o = b.options.expect("literal options");
        let p = o.producer.expect("producer");
        assert_eq!(p.module.as_deref(), Some("BroadwaySQS.Producer"));
        assert_eq!(p.options, ["queue_url", "max_number_of_messages", "visibility_timeout"]);
        assert_eq!(p.concurrency.unwrap().value, lit("2"));

        assert_eq!(o.processors.len(), 1);
        let pr = &o.processors[0];
        assert_eq!(pr.name, "default");
        assert_eq!(
            pr.concurrency.as_ref().unwrap().value,
            KnobValue::Env { key: ":order_processors".into(), default: Some("50".into()), required: false }
        );
        assert_eq!(pr.max_demand.as_ref().unwrap().value, lit("10"));

        let names: Vec<_> = o.batchers.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["default", "ledger"]);
        assert_eq!(o.batchers[1].batch_size.as_ref().unwrap().value, lit("50"));
        assert_eq!(o.batchers[1].batch_timeout.as_ref().unwrap().value, lit("5_000"));
        assert_eq!(o.batchers[0].range.start, 35, "a batcher's knobs light its own line");
    }

    #[test]
    fn name_module_resolves_to_the_module() {
        let o = info(&fixture()).options.unwrap();
        assert_eq!(o.name.unwrap().value, lit("MyApp.Orders.Pipeline"));
    }

    #[test]
    fn the_context_is_read_entry_by_entry() {
        let c = info(&fixture()).options.unwrap().context.unwrap();
        assert!(c.runtime.is_none());
        assert_eq!(c.entries[0].key, "source");
        assert_eq!(c.entries[0].value, lit(":orders"));
        assert_eq!(
            c.entries[1].value,
            KnobValue::Env { key: ":region".into(), default: Some("\"us-east-1\"".into()), required: false }
        );
    }

    #[test]
    fn each_handle_batch_clause_names_its_batcher() {
        let b = info(&fixture());
        let names: Vec<_> = b.batch_clauses.iter().map(|c| c.batcher.as_deref()).collect();
        assert_eq!(names, [Some("default"), Some("ledger")]);
    }

    #[test]
    fn options_built_elsewhere_are_runtime_not_guessed() {
        let src = "defmodule P do\n  use Broadway\n  def start_link(_), do: Broadway.start_link(__MODULE__, Config.opts())\n  def handle_message(_, m, _), do: m\nend\n";
        let b = info(src);
        assert!(b.options.is_none());
        assert_eq!(b.runtime_options.as_deref(), Some("Config.opts()"));
    }

    /// The shapes from `ImpactPipeline.Weather.Pipeline`: values from a struct,
    /// a context held in a variable, and one catch-all handle_batch.
    #[test]
    fn values_from_runtime_are_runtime() {
        let src = r#"defmodule W do
  use Broadway
  def start_link(_opts) do
    config = Config.load()
    Broadway.start_link(__MODULE__,
      name: __MODULE__,
      producer: [module: {BroadwaySQS.Producer, queue_url: url(config)}, concurrency: Application.get_env(:app, :producers, 2)],
      processors: [default: [concurrency: 100]],
      batchers: [default: [concurrency: Config.partitions(config), batch_size: 500]],
      context: %{config: config}
    )
  end
  def handle_message(_, m, _), do: m
  def handle_batch(_batcher, messages, _info, _ctx), do: messages
end
"#;
        let b = info(src);
        let o = b.options.unwrap();
        assert_eq!(
            o.batchers[0].concurrency.as_ref().unwrap().value,
            KnobValue::Runtime { text: "Config.partitions(config)".into() }
        );
        assert_eq!(o.context.unwrap().entries[0].value, KnobValue::Runtime { text: "config".into() });
        assert_eq!(b.batch_clauses[0].batcher, None, "a variable answers for every batcher");
    }

    #[test]
    fn a_comment_inside_the_producer_tuple_is_not_its_options() {
        let src = "defmodule P do\n  use Broadway\n  def start_link(_) do\n    Broadway.start_link(__MODULE__,\n      producer: [module: {\n        BroadwaySQS.Producer,\n        # why these attributes\n        queue_url: \"q\",\n        attribute_names: [:sent_timestamp]\n      }]\n    )\n  end\nend\n";
        let p = info(src).options.unwrap().producer.unwrap();
        assert_eq!(p.options, ["queue_url", "attribute_names"]);
    }

    #[test]
    fn a_module_without_use_broadway_has_no_view() {
        let o = elixir::parse("defmodule M do\n  def handle_message(a, b, c), do: b\nend\n").unwrap();
        assert!(o.modules[0].behaviour.is_none());
    }
}
