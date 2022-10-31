use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use dotenvy::dotenv;

use std::env;

pub mod models;
pub mod schema;

use models::*;
use schema::{documents, keywords};

pub type DbResult<T> = Result<T, diesel::result::Error>;

#[must_use]
pub fn get_connection_pool() -> Pool<ConnectionManager<PgConnection>> {
    dotenv().ok();
    let database_url =
        env::var("DATABASE_URL").expect("DATABASE_URL must be set!");
    let manager = ConnectionManager::<PgConnection>::new(database_url);
    Pool::builder()
        .test_on_check_out(true)
        .build(manager)
        .expect("Could not build connection pool")
}

pub fn insert_word(
    conn: &mut PgConnection,
    word: &String,
    document: &String,
) -> DbResult<()> {
    let keyword: Vec<Keyword> = keywords::dsl::keywords
        .filter(keywords::dsl::document.eq(document))
        .filter(keywords::dsl::word.eq(word))
        .load(conn)?;

    // If the keyword is already present, update its occurences count
    // and return
    if keyword.len() == 1 {
        let keyword = &keyword[0];
        diesel::update(keywords::dsl::keywords)
            .filter(keywords::dsl::id.eq(keyword.id))
            .set(keywords::dsl::occurrences.eq(keywords::dsl::occurrences + 1))
            .execute(conn)?;
        return Ok(());
    }

    // Insert the document if it isn’t already present in the database
    let doc: Vec<Document> = documents::dsl::documents
        .filter(documents::dsl::name.eq(document))
        .load(conn)?;
    if doc.is_empty() {
        diesel::insert_into(documents::dsl::documents)
            .values(Document {
                name: document.to_string(),
            })
            .execute(conn)?;
    }

    diesel::insert_into(keywords::dsl::keywords)
        .values((
            keywords::dsl::word.eq(word),
            keywords::dsl::document.eq(document.to_string()),
        ))
        .execute(conn)?;
    Ok(())
}

pub fn doc_list_keywords(
    conn: &mut PgConnection,
    document: &String,
) -> DbResult<Vec<String>> {
    use keywords::dsl;
    let mut keywords: Vec<(String, i32)> = dsl::keywords
        .filter(dsl::document.eq(document))
        .select((dsl::word, dsl::occurrences))
        .load(conn)?;
    keywords.sort_by_key(|k| k.1);
    let keywords: Vec<String> = keywords.iter().map(|k| k.0.clone()).collect();
    Ok(keywords)
}

pub fn keyword_list_docs(
    conn: &mut PgConnection,
    word: &String,
) -> DbResult<Vec<String>> {
    use keywords::dsl;
    let mut docs = dsl::keywords
        .filter(dsl::word.eq(word))
        .select((dsl::document, dsl::occurrences))
        .load::<(String, i32)>(conn)?;
    docs.sort_by_key(|k| k.1);
    let docs: Vec<String> = docs.iter().map(|s| s.0.clone()).collect();
    Ok(docs)
}

pub fn add_document(
    conn: &mut PgConnection,
    document: &String,
) -> DbResult<()> {
    use documents::dsl;
    diesel::insert_into(dsl::documents)
        .values(dsl::name.eq(document))
        .execute(conn)?;
    Ok(())
}

pub fn delete_document(
    conn: &mut PgConnection,
    document: &String,
) -> DbResult<()> {
    use documents::dsl;
    diesel::delete(dsl::documents.find(document)).execute(conn)?;
    Ok(())
}
