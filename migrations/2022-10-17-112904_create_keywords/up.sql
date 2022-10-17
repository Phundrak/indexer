-- Your SQL goes here
CREATE TABLE keywords (
  word VARCHAR NOT NULL PRIMARY KEY,
  occurrences INTEGER NOT NULL DEFAULT 1,
  document VARCHAR NOT NULL,
  FOREIGN KEY(document) REFERENCES documents(name)
)
