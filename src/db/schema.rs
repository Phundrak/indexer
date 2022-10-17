// @generated automatically by Diesel CLI.

diesel::table! {
    documents (name) {
        name -> Text,
    }
}

diesel::table! {
    keywords (word) {
        word -> Text,
        occurrences -> Integer,
        document -> Text,
    }
}

diesel::joinable!(keywords -> documents (document));

diesel::allow_tables_to_appear_in_same_query!(
    documents,
    keywords,
);
