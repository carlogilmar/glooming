-- A gloom is a note in a project's knowledge base, not a reading of a file set.
--
-- Everything that made a doc a *reading of a version* goes: the source
-- snapshots, their shas, the origin file and the set of files that joined it.
-- The code is always read live from disk now, so a gloom stores your prose and
-- nothing else, and the modules it covers are whatever that prose names.
--
-- The rows written before this were test glooms, and they are dropped rather
-- than migrated: their line references point at code that has moved since, and
-- there is no project to attach them to without guessing.
DROP TABLE doc_files;
DROP TABLE docs;

CREATE TABLE docs (
  id          INTEGER PRIMARY KEY,
  -- No ON DELETE CASCADE, deliberately: forgetting a folder from the recents
  -- list must never be the gesture that deletes everything written about it.
  project_id  INTEGER NOT NULL REFERENCES projects(id),
  title       TEXT NOT NULL,          -- empty until you name it
  label       TEXT,                   -- tags, comma-separated
  markdown    TEXT NOT NULL,
  created_at  TEXT NOT NULL,
  updated_at  TEXT NOT NULL
);

CREATE INDEX docs_project_idx ON docs(project_id, updated_at DESC);
