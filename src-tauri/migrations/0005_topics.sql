-- The Hub: a project's glooms as a table of contents.
--
-- A topic is a named section with a place in the order; a gloom sits in exactly
-- one topic (or none — "Unsorted"), at a place within it. One place per gloom,
-- because a table of contents needs one place and an order for each entry: tags
-- already cover "belongs to several", and a gloom listed under three topics
-- would be read three times by the newcomer the order is for. See
-- `mockup/hub.html`.
CREATE TABLE topics (
  id          INTEGER PRIMARY KEY,
  -- No cascade, for the same reason as docs: tidying the recents list must never
  -- be the gesture that deletes what you wrote.
  project_id  INTEGER NOT NULL REFERENCES projects(id),
  name        TEXT NOT NULL,
  position    INTEGER NOT NULL,
  created_at  TEXT NOT NULL
);
CREATE INDEX topics_project_idx ON topics(project_id, position);

-- NULL is Unsorted. Removing a topic sets its glooms back to NULL rather than
-- deleting them — `db::topics::delete` does that explicitly, in one transaction,
-- so a topic can never take a note with it.
ALTER TABLE docs ADD COLUMN topic_id INTEGER REFERENCES topics(id);
-- The place within its topic. Moving a gloom does not touch `updated_at`: Recent
-- is the order you WROTE in, and filing a note is not writing it.
ALTER TABLE docs ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
