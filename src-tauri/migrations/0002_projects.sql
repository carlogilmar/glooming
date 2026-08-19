-- Folders you have opened. A project is nothing more than a path you search
-- within: no scanning at startup, no index on disk, no notion of membership.
CREATE TABLE projects (
  id         INTEGER PRIMARY KEY,
  path       TEXT NOT NULL UNIQUE,   -- absolute directory
  name       TEXT NOT NULL,          -- last component, for display
  opened_at  TEXT NOT NULL           -- most recent use, for the recents list
);

CREATE INDEX projects_opened_at_idx ON projects(opened_at DESC);
