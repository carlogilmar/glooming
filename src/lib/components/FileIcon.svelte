<script lang="ts" module>
  /**
   * What a file is, for its icon — by name first, then extension.
   *
   * Deliberately few kinds. An editor's icon theme has hundreds, and at 14px most
   * of them are colour noise; what you scan a tree for is *Elixir or not*, *test
   * or not*, and *config, docs or data*. Those get a shape and a hue each, and
   * everything else is a plain page.
   */
  export type IconKind =
    | "folder"
    | "elixir"
    | "test"
    | "script"
    | "config"
    | "markdown"
    | "data"
    | "web"
    | "image"
    | "lock"
    | "file";

  const BY_NAME: Record<string, IconKind> = {
    "mix.exs": "config",
    "mix.lock": "lock",
    ".formatter.exs": "config",
    ".gitignore": "config",
    ".tool-versions": "config",
    ".credo.exs": "config",
    dockerfile: "config",
    makefile: "config",
  };

  const BY_EXT: Record<string, IconKind> = {
    ex: "elixir",
    exs: "script",
    heex: "web",
    eex: "web",
    leex: "web",
    md: "markdown",
    markdown: "markdown",
    txt: "markdown",
    json: "data",
    yml: "data",
    yaml: "data",
    toml: "data",
    csv: "data",
    sql: "data",
    js: "web",
    ts: "web",
    jsx: "web",
    tsx: "web",
    css: "web",
    scss: "web",
    html: "web",
    svelte: "web",
    png: "image",
    jpg: "image",
    jpeg: "image",
    gif: "image",
    svg: "image",
    webp: "image",
    ico: "image",
    lock: "lock",
  };

  export function iconFor(name: string): IconKind {
    const lower = name.toLowerCase();
    if (BY_NAME[lower]) return BY_NAME[lower];
    // A suite is the one Elixir file whose role is in its name.
    if (lower.endsWith("_test.exs")) return "test";
    const dot = lower.lastIndexOf(".");
    return (dot > 0 && BY_EXT[lower.slice(dot + 1)]) || "file";
  }
</script>

<script lang="ts">
  let { kind, open = false }: { kind: IconKind; open?: boolean } = $props();
</script>

<!-- 14px, drawn on a 16-unit grid. Colour comes from `currentColor` on a class
     per kind, so both themes are handled by tokens rather than by the paths. -->
<svg class="ico {kind}" viewBox="0 0 16 16" width="14" height="14" aria-hidden="true">
  {#if kind === "folder"}
    {#if open}
      <path d="M1.5 4.5v8h11l2-5.5H4.2L2.5 12" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
      <path d="M1.5 4.5V3.2h4l1.2 1.3h5.8v2.5" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
    {:else}
      <path d="M1.5 3.2h4l1.2 1.3h7.8v8.3h-13z" fill="currentColor" fill-opacity=".18" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
    {/if}
  {:else if kind === "elixir" || kind === "script" || kind === "test"}
    <!-- The drop. Solid for a module, hollow for a script, and a test carries a
         check — the three Elixir files you tell apart at a glance. -->
    <path
      d="M8 1.5C6 4.2 3.8 6.9 3.8 9.9a4.2 4.2 0 0 0 8.4 0C12.2 6.9 10 4.2 8 1.5z"
      fill={kind === "elixir" ? "currentColor" : "none"}
      fill-opacity={kind === "elixir" ? ".85" : "0"}
      stroke="currentColor"
      stroke-width="1.2"
      stroke-linejoin="round"
    />
    {#if kind === "test"}
      <path d="M6 10l1.4 1.4L10.2 8.6" fill="none" stroke="currentColor" stroke-width="1.3" stroke-linecap="round" stroke-linejoin="round" />
    {/if}
  {:else}
    <!-- A page with a folded corner, and a mark inside that says what kind. -->
    <path d="M3.5 1.5h6l3 3v10h-9z" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
    <path d="M9.5 1.5v3h3" fill="none" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round" />
    {#if kind === "config"}
      <circle cx="8" cy="10" r="1.9" fill="none" stroke="currentColor" stroke-width="1.2" />
    {:else if kind === "markdown"}
      <path d="M5.5 8.5h5M5.5 10.5h5M5.5 12.5h3" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" />
    {:else if kind === "data"}
      <path d="M6.8 8.2c-1 0-1 .9-1 1.6s-.5 1-.9 1c.4 0 .9.3.9 1s0 1.6 1 1.6M9.2 8.2c1 0 1 .9 1 1.6s.5 1 .9 1c-.4 0-.9.3-.9 1s0 1.6-1 1.6" fill="none" stroke="currentColor" stroke-width="1" />
    {:else if kind === "web"}
      <path d="M6.6 8.6L5 10.6l1.6 2M9.4 8.6l1.6 2-1.6 2" fill="none" stroke="currentColor" stroke-width="1.1" stroke-linecap="round" stroke-linejoin="round" />
    {:else if kind === "image"}
      <path d="M5 13l2.2-2.6 1.5 1.6 1-1.1L11 13z" fill="currentColor" fill-opacity=".7" />
      <circle cx="9.8" cy="8.4" r=".9" fill="currentColor" />
    {:else if kind === "lock"}
      <rect x="5.6" y="9.6" width="4.8" height="3.6" rx=".6" fill="none" stroke="currentColor" stroke-width="1.1" />
      <path d="M6.6 9.6V8.5a1.4 1.4 0 0 1 2.8 0v1.1" fill="none" stroke="currentColor" stroke-width="1.1" />
    {/if}
  {/if}
</svg>

<style>
  .ico {
    flex: none;
    display: block;
    color: var(--fg-faint);
  }
  /* Elixir's own violet for the language — the syntax keyword colour, which is
     already the app's "this is Elixir" hue in the code pane. */
  .elixir,
  .script {
    color: var(--syn-kw);
  }
  .test {
    color: var(--pub);
  }
  .folder {
    color: var(--gloom-deep);
  }
  .config {
    color: var(--priv);
  }
  .markdown {
    color: var(--accent);
  }
  .data,
  .web {
    color: var(--syn-at);
  }
</style>
