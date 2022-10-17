use super::schema::*;
use diesel::prelude::*;

#[derive(Queryable, Insertable)]
pub struct Document {
    pub name: String,
}

#[derive(Queryable, Insertable)]
pub struct Keyword {
    pub word: String,
    pub occurrences: i32,
    pub document: String,
}
