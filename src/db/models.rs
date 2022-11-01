use crate::db::schema::{documents, keywords};
use diesel::prelude::*;
use rocket::serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Queryable,
    Insertable,
    Hash,
    Deserialize,
    Serialize,
    Clone,
    PartialEq,
    Eq,
)]
#[serde(crate = "rocket::serde")]
pub struct Document {
    pub name: String,
    pub title: String,
}

#[derive(Debug, Queryable, Insertable)]
pub struct Keyword {
    pub id: i32,
    pub word: String,
    pub occurrences: i32,
    pub document: String,
}
