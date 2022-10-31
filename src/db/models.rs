use diesel::prelude::*;
use crate::db::schema::{documents, keywords};

#[derive(Debug, Queryable, Insertable)]
pub struct Document {
    pub name: String,
}

#[derive(Debug, Queryable, Insertable)]
pub struct Keyword {
    pub id: i32,
    pub word: String,
    pub occurrences: i32,
    pub document: String,
}
