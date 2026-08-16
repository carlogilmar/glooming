-- One table. A `doc` is one reading of one file: the source as it was when you
-- read it, plus the markdown you wrote about it.
CREATE TABLE docs (
  id          INTEGER PRIMARY KEY,
  path        TEXT NOT NULL,          -- absolute path as opened
  filename    TEXT NOT NULL,          -- denormalized for the library list
  lang        TEXT NOT NULL,          -- "elixir"
  title       TEXT NOT NULL,          -- seeded from the module name, editable
  branch      TEXT,                   -- label, prefilled from .git/HEAD
  label       TEXT,                   -- free field: "PR #412", "claude-generated"
  markdown    TEXT NOT NULL,          -- the doc, extended syntax and all
  source      TEXT NOT NULL,          -- snapshot of the file when read
  source_sha  TEXT NOT NULL,          -- sha256 of `source`
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

CREATE INDEX docs_path_idx       ON docs(path);
CREATE INDEX docs_updated_at_idx ON docs(updated_at DESC);
