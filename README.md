<div align="center">

<img src="app-icon.png" alt="Glooming" width="140" />

# Glooming

</div>

A desktop tool for reading an Elixir codebase deeply. Open a project, browse its
modules beside live views of what each one is and touches, and write what you
work out as **glooms** — notes whose references walk the code in the order your
prose takes it. Nothing is copied out of your code, and nothing is ever written
into it.

## Requirements

| Tool | Version | Why |
|---|---|---|
| [asdf](https://asdf-vm.com) | any | pins the toolchain via `.tool-versions` |
| Rust | 1.95.0 | Tauri backend, tree-sitter parsing |
| Node.js | 25.9.0 | frontend build |
| pnpm | 9.15.0 | **the only package manager** — a `package-lock.json` is a bug |
| Xcode Command Line Tools | — | macOS only; `xcode-select --install` |

## Getting started

```bash
git clone <repo-url> glooming
cd glooming

# Install the pinned toolchain. Plugins are one-time per machine.
asdf plugin add rust && asdf plugin add nodejs && asdf plugin add pnpm
asdf install

pnpm install          # JS deps; cargo fetches Rust deps on first build
pnpm tauri dev        # run the app
```

The first `pnpm tauri dev` compiles the Rust side from scratch — a few minutes.

> **`Port 1420 is already in use`** means an orphaned Vite server survived a
> previous run: `lsof -ti:1420 | xargs kill -9`

`CLAUDE.md` is the map of how it all fits together.
