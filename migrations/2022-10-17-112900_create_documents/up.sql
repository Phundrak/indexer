-- Your SQL goes here
CREATE TYPE DocumentType AS ENUM ('online', 'offline');

CREATE TABLE documents (
  name VARCHAR NOT NULL PRIMARY KEY,
  title VARCHAR NOT NULL,
  doctype DocumentType NOT NULL,
  description TEXT NOT NULL
)
