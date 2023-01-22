use diesel::prelude::*;
use diesel::r2d2::{ConnectionManager, Pool};
use diesel::PgConnection;
use diesel_migrations::{
    embed_migrations, EmbeddedMigrations, MigrationHarness,
};

use dotenvy::dotenv;
use tracing::debug;

use std::collections::HashMap;

pub mod models;
pub mod schema;

use models::{Document, Keyword};
use schema::{documents, keywords};

use crate::fileparser::ParsedDocument;

pub type DatabaseResult<T> = Result<T, diesel::result::Error>;

/// List of migrations the database may have to perform when indexer
/// is launching
pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();

/// Run the list of migrations held by `MIGRATIONS`.
///
/// # Errors
///
/// If any error is encountered while running a migration, return
pub fn run_migrations(
    connection: &mut impl MigrationHarness<diesel::pg::Pg>,
) -> DatabaseResult<()> {
    use diesel::result::{DatabaseErrorKind, Error};
    match connection.has_pending_migration(MIGRATIONS) {
        Ok(migrate) => {
            if migrate {
                connection
                    .run_next_migration(MIGRATIONS)
                    .map(|_| ())
                    .map_err(|e| {
                        Error::DatabaseError(
                            DatabaseErrorKind::Unknown,
                            Box::new(format!("Error running migrations: {e}")),
                        )
                    })
            } else {
                Ok(())
            }
        }
        Err(e) => Err(Error::DatabaseError(
            DatabaseErrorKind::Unknown,
            Box::new(format!("Error: {e}")),
        )),
    }
}

#[must_use]
pub fn get_connection_pool() -> Pool<ConnectionManager<PgConnection>> {
    dotenv().ok();
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set!");
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

use crate::server::RankedKeyword;
/// List keywords associated with a document
///
/// # Errors
///
/// Errors may be returned by Diesel, forward them to the function
/// calling `doc_list_keywords`.
pub fn doc_list_keywords(
    conn: &mut PgConnection,
    document: &str,
) -> DatabaseResult<Vec<RankedKeyword>> {
    use keywords::dsl;
    let mut keywords: Vec<RankedKeyword> = dsl::keywords
        .filter(dsl::document.eq(document))
        .select((dsl::word, dsl::occurrences))
        .load::<(String, i32)>(conn)?
        .iter()
        .map(|k| RankedKeyword {
            keyword: k.0.clone(),
            rank: k.1,
        })
        .collect();
    keywords.sort_by_key(|k| k.rank);
    keywords.reverse();
    Ok(keywords)
}

use crate::server::RankedDoc;

use self::models::DocType;

/// Search a document by keywords
///
/// Return the documents matching at least one of the `words`, ordered
/// in descending order by the amount of hits per word.
///
/// # Errors
///
/// Errors may be returned by Diesel, forward them to the function
/// calling `keywords_search`.
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
        debug!("Documents for query {words:?}: {list:?}");
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
            hits: Some(k.1.to_owned()),
            ..k.0.clone().into()
        })
        .collect::<Vec<RankedDoc>>())
}

/// Add a document to the indexer
///
/// Add a document’s description to the database as well as its
/// keywords
///
/// # Errors
///
/// Errors may be returned by Diesel, forward them to the function
/// calling `add_document`.
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

/// List documents indexed in the database
///
/// # Errors
///
/// If any error is returned by the database, forward it to the
/// function calling `list_documents`
pub fn list_documents(
    conn: &mut PgConnection,
) -> DatabaseResult<Vec<Document>> {
    use documents::dsl;
    dsl::documents.load(conn)
}

/// Delete a document from the database
///
/// # Errors
///
/// If any error is returned by the database, forward it to the
/// function calling `list_documents`
pub fn delete_document(
    conn: &mut PgConnection,
    document: &str,
) -> DatabaseResult<()> {
    use documents::dsl;
    diesel::delete(dsl::documents.find(document)).execute(conn)?;
    Ok(())
}

/// Get a specific document from database
///
/// Get a document by name in the database and return it as-is to the
/// caller function.
pub fn get_document(conn: &mut PgConnection, id: &str) -> Option<Document> {
    use documents::dsl;
    match dsl::documents.find(id.to_string()).first::<Document>(conn) {
        Ok(document) => Some(document),
        Err(diesel::NotFound) => None,
        Err(e) => {
            info!("Failed to retrieve document {id} from database: {e:?}");
            None
        }
    }
}

/// Retrieve the S3 filename of a document
///
/// If a document’s primary key matches the argument `id` and that
/// document is a document stored on the S3 remote storage, return its
/// filename.
pub fn get_s3_filename(conn: &mut PgConnection, id: &str) -> Option<String> {
    if let Some(document) = get_document(conn, id) {
        if document.doctype == DocType::Online {
            Some(document.name)
        } else {
            None
        }
    } else {
        None
    }
}
