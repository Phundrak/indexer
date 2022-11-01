use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use dotenvy::dotenv;

use std::collections::HashMap;

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

pub fn get_document(
    conn: &mut PgConnection,
    name: &String,
) -> DbResult<Document> {
    documents::dsl::documents.find(name).first(conn)
}

pub fn insert_word(
    conn: &mut PgConnection,
    word: &String,
    document: Document,
) -> DbResult<()> {
    let keyword: Vec<Keyword> = keywords::dsl::keywords
        .filter(keywords::dsl::document.eq(document.name.to_owned()))
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
        .filter(documents::dsl::name.eq(document.name.to_owned()))
        .load(conn)?;
    if doc.is_empty() {
        diesel::insert_into(documents::dsl::documents)
            .values(document.to_owned())
            .execute(conn)?;
    }

    diesel::insert_into(keywords::dsl::keywords)
        .values((
            keywords::dsl::word.eq(word),
            keywords::dsl::document.eq(document.name),
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
    keywords.reverse();
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
    docs.reverse();
    let docs: Vec<String> = docs.iter().map(|s| s.0.clone()).collect();
    Ok(docs)
}

use crate::server::RankedDoc;
pub fn keywords_search(
    conn: &mut PgConnection,
    words: &[String],
) -> DbResult<Vec<RankedDoc>> {
    let mut docs: HashMap<Document, i32> = HashMap::new();
    for word in words {
        let list = keywords::table
            .left_join(
                documents::table.on(keywords::document
                    .eq(documents::name)
                    .and(keywords::word.eq(word))),
            )
            .select((
                documents::name.nullable(),
                documents::title.nullable(),
                keywords::occurrences,
            ))
            .load::<(Option<String>, Option<String>, i32)>(conn)?
            .iter()
            .map(|item| {
                (
                    Document {
                        name: item.clone().0.unwrap(),
                        title: item.clone().1.unwrap(),
                    },
                    item.2,
                )
            })
            .collect::<Vec<(Document, i32)>>();
        for item in list {
            docs.entry(item.0)
                .and_modify(|occ| *occ += item.1)
                .or_insert(item.1);
        }
    }
    let mut docs: Vec<(Document, i32)> = docs
        .iter()
        .map(|(doc, occ)| (doc.to_owned(), occ.to_owned()))
        .collect();
    docs.sort_by_key(|k| k.1);
    docs.reverse();
    Ok(docs
        .iter()
        .map(|k| RankedDoc {
            doc: k.0.name.to_owned(),
            title: k.0.title.to_owned(),
            hits: k.1.to_owned(),
        })
        .collect::<Vec<RankedDoc>>())
}

pub fn add_document(
    conn: &mut PgConnection,
    document: Document,
) -> DbResult<()> {
    use documents::dsl;
    diesel::insert_into(dsl::documents)
        .values(document)
        .execute(conn)?;
    Ok(())
}

pub fn list_documents(conn: &mut PgConnection) -> DbResult<Vec<String>> {
    use documents::dsl;
    dsl::documents.select(dsl::name).load(conn)
}

pub fn delete_document(
    conn: &mut PgConnection,
    document: &String,
) -> DbResult<()> {
    use documents::dsl;
    diesel::delete(dsl::documents.find(document)).execute(conn)?;
    Ok(())
}
