use crate::db::schema::{documents, keywords};
use diesel::prelude::*;
use rocket::serde::{Deserialize, Serialize};

#[derive(
    Debug,
    Serialize,
    Deserialize,
    Clone,
    PartialEq,
    Eq,
    Copy,
    diesel_derive_enum::DbEnum,
    Hash,
)]
#[DieselTypePath = "crate::db::schema::sql_types::Documenttype"]
#[serde(crate = "rocket::serde")]
pub enum DocumentType {
    Online,
    Offline,
}

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
    pub doctype: DocumentType,
    pub description: String,
}

#[derive(Debug, Queryable, Insertable)]
pub struct Keyword {
    pub id: i32,
    pub word: String,
    pub occurrences: i32,
    pub document: String,
}
