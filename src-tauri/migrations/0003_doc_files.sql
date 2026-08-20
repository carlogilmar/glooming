-- A reading is a set of files, not one.
--
-- The file set lives in rows, not in the markdown — deliberately, and for the
-- same reason a single-file doc's `path` has always been a column: the note has
-- never declared which file it was about, so putting a *set* of paths into the
-- prose would be a new inconsistency rather than a preserved principle. The
-- module-qualified references in the prose already tell a reader which modules
-- a reading covers.
--
-- One row per file means one snapshot per file, so **staleness is per-file**.
-- A single `docs.source` could only ever say "something changed".
CREATE TABLE doc_files (
  id          INTEGER PRIMARY KEY,
  doc_id      INTEGER NOT NULL REFERENCES docs(id) ON DELETE CASCADE,
  path        TEXT NOT NULL,          -- absolute path as opened
  filename    TEXT NOT NULL,          -- denormalized, for the strip
  lang        TEXT NOT NULL,
  source      TEXT NOT NULL,          -- snapshot when this file joined
  source_sha  TEXT NOT NULL,
  -- The order you opened them, which is the order the strip shows. Not the
  -- order of the reading: that is whatever order your prose takes.
  position    INTEGER NOT NULL,
  added_at    TEXT NOT NULL,
  UNIQUE (doc_id, path)
);

CREATE INDEX doc_files_doc_idx  ON doc_files(doc_id, position);
CREATE INDEX doc_files_path_idx ON doc_files(path);

-- Every doc written before this migration is a one-file reading. `docs.path`
-- stays the *origin* — the file the doc was seeded from, whose module owns the
-- `lgtm:functions` block and which the library groups by — so nothing that
-- reads `docs` needs to change.
INSERT INTO doc_files (doc_id, path, filename, lang, source, source_sha, position, added_at)
SELECT id, path, filename, lang, source, source_sha, 0, created_at FROM docs;
