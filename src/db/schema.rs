// @generated automatically by Diesel CLI.

pub mod sql_types {
    #[derive(diesel::sql_types::SqlType)]
    #[diesel(postgres_type(name = "documenttype"))]
    pub struct Documenttype;
}

diesel::table! {
    use diesel::sql_types::*;
    use super::sql_types::Documenttype;

    documents (name) {
        name -> Varchar,
        title -> Varchar,
        doctype -> Documenttype,
        description -> Text,
    }
}

diesel::table! {
    keywords (id) {
        id -> Int4,
        word -> Varchar,
        occurrences -> Int4,
        document -> Varchar,
    }
}

diesel::joinable!(keywords -> documents (document));

diesel::allow_tables_to_appear_in_same_query!(
    documents,
    keywords,
);
