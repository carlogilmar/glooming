// The ```lgtm:settings block: what a config script configures.
//
// Ported from `mockup/kinds.html`, which is the visual contract.
//
// A config file has no functions, so surface, treemap and reach all say nothing
// about it. It is a key–value tree, and there is no honest chart in it — the
// value is in the grouping and in one marking: **which values come from the
// environment**. That is the difference between something you can change at
// deploy time and something baked into the release, and in sixty lines of
// keyword lists it is invisible.

function esc(s: string): string {
  return s.replace(/[&<>"]/g, (c) =>
    c === "&" ? "&amp;" : c === "<" ? "&lt;" : c === ">" ? "&gt;" : "&quot;",
  );
}

export interface Setting {
  key: string;
  line: number;
  endLine: number;
  /** `env` | `env!` | `secret` | `literal` */
  source: "env" | "env!" | "secret" | "literal";
  /** The variable name for env, or the value for a literal. */
  detail: string;
}

export interface Group {
  app: string;
  target: string | null;
  line: number;
  endLine: number;
  settings: Setting[];
}

export interface Settings {
  groups: Group[];
  imports: string[];
}

/**
 * Two levels by indent, like `lgtm:deps`:
 *
 *     :my_app MyApp.Repo : 3
 *       username : 4 = "postgres"
 *       password : 5 env! DB_PASSWORD
 *     import_config : dev.secret.exs
 */
export function parseSettings(body: string): Settings {
  const out: Settings = { groups: [], imports: [] };

  for (const raw of body.split("\n")) {
    if (!raw.trim() || raw.trim().startsWith("#")) continue;
    const colon = raw.indexOf(":", raw.search(/\S/) + 1);
    if (colon < 1) continue;

    const label = raw.slice(0, colon).trim();
    const value = raw.slice(colon + 1).trim();
    if (!label) continue;

    if (label === "import_config") {
      out.imports.push(value);
      continue;
    }

    const indent = raw.length - raw.trimStart().length;
    if (indent >= 4 && out.groups.length) {
      // `4 env! DB_PASSWORD` / `4-6 = [...]` / `7 secret`
      const [lineNo, ...rest] = value.split(/\s+/);
      const [startTxt, endTxt] = lineNo.split("-");
      const tail = rest.join(" ");
      let source: Setting["source"] = "literal";
      let detail = tail;

      if (tail.startsWith("env!")) {
        source = "env!";
        detail = tail.slice(4).trim();
      } else if (tail.startsWith("env")) {
        source = "env";
        detail = tail.slice(3).trim();
      } else if (tail === "secret") {
        source = "secret";
        detail = "";
      } else if (tail.startsWith("=")) {
        detail = tail.slice(1).trim();
      }

      const line = parseInt(startTxt, 10) || 0;
      out.groups[out.groups.length - 1].settings.push({
        key: label,
        line,
        endLine: parseInt(endTxt ?? startTxt, 10) || line,
        source,
        detail,
      });
    } else {
      // `:my_app MyApp.Repo` — app, then an optional target.
      const [app, ...target] = label.split(/\s+/);
      const [startTxt, endTxt] = value.split(/\s+/)[0].split("-");
      const line = parseInt(startTxt, 10) || 0;
      out.groups.push({
        app,
        target: target.length ? target.join(" ") : null,
        line,
        endLine: parseInt(endTxt ?? startTxt, 10) || line,
        settings: [],
      });
    }
  }

  return out;
}

/** `data-line` plus `data-end`, so a click can select the whole setting. */
function span(line: number, endLine: number): string {
  if (!line) return "";
  return ` data-line="${line}" data-end="${Math.max(endLine, line)}"`;
}

function settingHtml(s: Setting): string {
  const env = s.source === "env" || s.source === "env!";
  const cls = ["kv", env ? "fromenv" : "", s.source === "secret" ? "secret" : ""]
    .filter(Boolean)
    .join(" ");

  const value = env
    ? `<span class="from">${s.source === "env!" ? "env!" : "env"}</span>${esc(s.detail)}`
    : s.source === "secret"
      ? // The key is listed — that it is hardcoded is the finding — but the
        // value never rides along, because docs get pasted into PR comments.
        "set in this file — value hidden"
      : esc(s.detail);

  return (
    `<div class="${cls}"${span(s.line, s.endLine)} data-sig="${esc(s.key)}">` +
    `<span class="k">${esc(s.key)}</span><span class="v">${value}</span>` +
    `</div>`
  );
}

export function renderSettings(body: string, file: string): string {
  const s = parseSettings(body);
  if (!s.groups.length) {
    return `<div class="lgtm-settings empty">Empty settings block — re-seed this doc, or write <code>:my_app MyApp.Repo : 3</code> style rows here.</div>`;
  }

  const total = s.groups.reduce((n, g) => n + g.settings.length, 0);
  const apps = new Set(s.groups.map((g) => g.app));

  const groups = s.groups
    .map((g) => {
      const envs = g.settings.filter((x) => x.source === "env" || x.source === "env!").length;
      return (
        `<div class="grp">` +
        `<div class="head"${span(g.line, g.endLine)} data-sig="${esc(g.app)} ${esc(g.target ?? "")}">` +
        `<span class="app">${esc(g.app)}</span>` +
        (g.target ? `<span class="sep">›</span><span class="target">${esc(g.target)}</span>` : "") +
        (envs ? `<span class="envn">⚡ ${envs}</span>` : "") +
        `<span class="n">${g.settings.length}</span>` +
        `</div>` +
        g.settings.map(settingHtml).join("") +
        `</div>`
      );
    })
    .join("");

  // Override order is the single most common config confusion, so the chain
  // says which way it runs.
  const chain = s.imports.length
    ? `<footer><span class="lbl">load chain</span>` +
      `<span class="path"><b>${esc(file)}</b> → ${s.imports.map(esc).join(" → ")}</span>` +
      `<span class="wins">later wins</span></footer>`
    : "";

  return (
    `<div class="lgtm-settings">` +
    `<header><span class="tag">lgtm:settings</span><span>${esc(file)}</span>` +
    `<span class="count">${apps.size} ${apps.size === 1 ? "app" : "apps"} · ${total} settings</span></header>` +
    groups +
    chain +
    `</div>`
  );
}
