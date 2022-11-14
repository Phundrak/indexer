// @generated automatically by Diesel CLI.

diesel::table! {
    documents (name) {
        name -> Varchar,
        title -> Varchar,
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
