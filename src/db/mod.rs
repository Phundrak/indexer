use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use dotenvy::dotenv;
use tracing::debug;

use std::collections::HashMap;

use std::env;

pub mod models;
pub mod schema;

use models::{Document, Keyword};
use schema::{documents, keywords};

use crate::fileparser::ParsedDocument;

pub type DatabaseResult<T> = Result<T, diesel::result::Error>;

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

/// Insert a keyword in the database
///
/// Insert the keyword `word` associated with the document `doc` in
/// the database. This function assumes the document already exists.
///
/// # Errors
///
/// If the document does not exist in the database, a
/// `DocumentMissing` error is returned (see [`DbError`]). If any
/// other error is thrown by Diesel, it is wrapped in a `Other` error.
///
/// [`DbError`]: ./enum.DbError.html
pub fn insert_word(
    conn: &mut PgConnection,
    word: &str,
    doc: &str,
    weight: Option<i32>,
) -> DatabaseResult<()> {
    use keywords::dsl;
    // Verify if the document exists before inserting keywords
    if documents::dsl::documents
        .find(doc)
        .first::<Document>(conn)
        .is_err()
    {
        return Err(diesel::result::Error::NotFound);
    }

    if let Ok(val) = keywords::dsl::keywords
        .filter(dsl::document.eq(doc))
        .filter(dsl::word.eq(word))
        .first::<Keyword>(conn)
    {
        diesel::update(dsl::keywords.find(val.id))
            .set(dsl::occurrences.eq(val.occurrences + weight.unwrap_or(1)))
            .execute(conn)?;
    } else {
        diesel::insert_into(keywords::dsl::keywords)
            .values((
                dsl::word.eq(word),
                dsl::document.eq(doc),
                dsl::occurrences.eq(weight.unwrap_or(1)),
            ))
            .execute(conn)?;
    }
    Ok(())
}

pub fn doc_list_keywords(
    conn: &mut PgConnection,
    document: &str,
) -> DatabaseResult<Vec<String>> {
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

use crate::server::RankedDoc;
pub fn keywords_search(
    conn: &mut PgConnection,
    words: &[String],
) -> DatabaseResult<Vec<RankedDoc>> {
    let mut docs: HashMap<Document, i32> = HashMap::new();
    for word in words {
        let list = keywords::table
            .left_join(
                documents::table.on(keywords::document
                    .eq(documents::name)
                    .and(keywords::word.eq(word))),
            )
            .load::<(Keyword, Option<Document>)>(conn)?
            .iter()
            .filter_map(|item| {
                item.1.as_ref().map(|doc| (doc.clone(), item.0.occurrences))
            })
            .collect::<Vec<(Document, i32)>>();
        debug!("Documents for query {:?}: {:?}", words, list);
        for item in list {
            docs.entry(item.0)
                .and_modify(|occ| *occ += item.1)
                .or_insert(item.1);
        }
    }
    let mut docs: Vec<(Document, i32)> = docs
        .iter()
        .map(|(doc, occ)| (doc.clone(), occ.to_owned()))
        .collect();
    docs.sort_by_key(|k| k.1);
    docs.reverse();
    Ok(docs
        .iter()
        .map(|k| RankedDoc {
            doc: k.0.name.clone(),
            title: k.0.title.clone(),
            hits: k.1.to_owned(),
        })
        .collect::<Vec<RankedDoc>>())
}

pub fn add_document(
    conn: &mut PgConnection,
    document: &Document,
    content: &ParsedDocument,
) -> DatabaseResult<()> {
    use documents::dsl;
    diesel::insert_into(dsl::documents)
        .values(document.clone())
        .execute(conn)?;
    for keyword in &content.keywords {
        insert_word(conn, keyword, &document.name, Some(2))?;
    }
    for keyword in &content.content {
        insert_word(conn, keyword, &document.name, None)?;
    }
    Ok(())
}

pub fn list_documents(conn: &mut PgConnection) -> DatabaseResult<Vec<String>> {
    use documents::dsl;
    dsl::documents.select(dsl::name).load(conn)
}

pub fn delete_document(
    conn: &mut PgConnection,
    document: &str,
) -> DatabaseResult<()> {
    use documents::dsl;
    diesel::delete(dsl::documents.find(document)).execute(conn)?;
    Ok(())
}
