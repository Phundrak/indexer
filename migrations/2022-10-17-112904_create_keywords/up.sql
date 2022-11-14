-- Your SQL goes here
CREATE TABLE keywords (
  id SERIAL PRIMARY KEY,
  word VARCHAR NOT NULL,
  occurrences INTEGER NOT NULL DEFAULT 1,
  document VARCHAR
           REFERENCES documents(name)
           ON UPDATE CASCADE
           ON DELETE CASCADE
           NOT NULL
)
