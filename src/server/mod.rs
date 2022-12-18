use std::collections::HashMap;

use color_eyre::eyre::Result;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use rocket::fs::TempFile;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::State;
use tracing::{debug, info};

use crate::db::models::DocType;
use crate::db::{self, models::Document};
use crate::fileparser::get_content;
use crate::kwparser;
use crate::spelling::Dictionary;

use self::appwrite::UserSession;

pub mod s3;
mod appwrite;

extern crate s3 as s3rust;

type DbPool = PooledConnection<ConnectionManager<PgConnection>>;

#[allow(clippy::module_name_repetitions)]
pub struct ServerState {
    pub dictionary: Option<Dictionary>,
    pub glaff: Option<HashMap<String, String>>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub stopwords: Vec<String>,
    pub s3_bucket: s3rust::Bucket,
    pub appwrite_endpoint: String,
    pub appwrite_project: String,
    pub appwrite_key: String,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct RankedDoc {
    pub doc: String,
    pub title: String,
    pub description: String,
    pub hits: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct RankedKeyword {
    pub keyword: String,
    pub rank: i32,
}

#[derive(Serialize, Default)]
#[serde(crate = "rocket::serde")]
pub struct QueryResult {
    spelling_suggestion: Option<String>,
    results: Vec<RankedDoc>,
    using_suggestion: bool,
}

impl QueryResult {
    #[must_use]
    pub fn new(
        results: Vec<RankedDoc>,
        spelling_suggestion: Option<String>,
        using_suggestion: &UseSpellingSuggestion,
    ) -> Self {
        Self {
            results,
            spelling_suggestion,
            using_suggestion: match using_suggestion {
                UseSpellingSuggestion::Yes => true,
                UseSpellingSuggestion::No => false,
            },
        }
    }
}

pub enum UseSpellingSuggestion {
    Yes,
    No,
}

macro_rules! api_error {
    ($message:expr) => {
        Custom(Status::InternalServerError, $message)
    };
}

macro_rules! get_connector {
    ($db:expr) => {
        match $db.pool.get() {
            Ok(val) => val,
            Err(_) => {
                return Err(api_error!(
                    "Failed to connect to the database".to_owned()
                ));
            }
        }
    };
}

macro_rules! json_val_or_error {
    ($result:expr) => {
        $result
            .map(|val| Json(val))
            .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
    };
}

pub type ApiResponse<T> = std::result::Result<T, Custom<String>>;

async fn fetch_content(url: &String) -> ApiResponse<Vec<u8>> {
    match reqwest::get(url).await {
        Ok(val) => match val.bytes().await {
            Ok(val) => Ok(val.into()),
            Err(e) => Err(Custom(
                Status::NotAcceptable,
                format!("Cannot retrieve bytes from requested document; {}", e),
            )),
        },
        Err(e) => Err(Custom(Status::InternalServerError, e.to_string())),
    }
}

// Inserting into the database ////////////////////////////////////////////////

fn index_file(
    state: &State<ServerState>,
    file: &[u8],
    identifier: &str,
    file_type: DocType,
) -> ApiResponse<()> {
    let stop_words = &state.stopwords;
    let glaff = &state.glaff;
    let content = get_content(file, stop_words, glaff)
        .map_err(|e| Custom(Status::NotAcceptable, format!("{:?}", e)))?;
    debug!("{:?}", content);
    let conn = &mut state.pool.get().map_err(|e| {
        api_error!(format!("Failed to connect to the database: {}", e))
    })?;
    info!("== Inserting {} in database", &identifier);

    let doc = Document {
        title: content.title.clone(),
        name: identifier.to_string(),
        doctype: file_type,
        description: content.description.clone(),
    };
    db::add_document(conn, &doc, &content).map_err(|e| {
        Custom(
            Status::InternalServerError,
            format!("Failed to insert URL {} as a document: {}", identifier, e),
        )
    })?;
    info!("Indexed {}", identifier);
    Ok(())
}

/// Generate a simple 500 error
///
/// Just wrap an error as a string in a Rocket 500 status.
///
/// The needless pass by value is allowed here since the function can
/// be called in a simple manner with
/// `.map_err(simple_internal_error)`, which won’t allow to pass `e`
/// as a reference.
#[allow(clippy::needless_pass_by_value)]
fn simple_internal_error<E>(e: E) -> Custom<String>
where
    E: ToString,
{
    Custom(Status::InternalServerError, e.to_string())
}

async fn file_to_vec(mut file: TempFile<'_>) -> ApiResponse<Vec<u8>> {
    file.move_copy_to("tmp/file")
        .await
        .map_err(simple_internal_error)?;
    let file = std::fs::read("tmp/file").map_err(simple_internal_error)?;
    std::fs::remove_file("tmp/file").map_err(simple_internal_error)?;
    debug!("Deleted temporary file");
    Ok(file)
}

/// Upload and index a document
///
/// The `file` transmitted as pure data is uploaded to a S3 bucket and
/// then parsed. If any error arise when indexing the document, the
/// object on the S3 bucket is then deleted. Otherwise, its name on
/// the bucket, its sha256 sum concatenated with its filename, is
/// stored as the document’s name.
///
/// # Errors
///
/// If any error arise from the indexation of the file, if the file
/// fails to upload to the S3 bucket or fails to be deleted from it,
/// the error is wrapped in a 500 Rocket error and returned to the
/// user. For more information, see `s3::upload_fle`,
/// `s3::delete_file`, and `index_file`.
#[post("/docs/file/<filename>", data = "<file>")]
pub async fn index_upload(
    state: &State<ServerState>,
    file: TempFile<'_>,
    filename: String,
    _auth: UserSession<'_>,
) -> ApiResponse<()> {
    use sha256::digest;
    let file = file_to_vec(file).await?;
    let id = digest(&file as &[u8]);
    let filename = format!("{}-{}", id, filename);

    info!("Uploading file {}", filename);
    s3::upload_file(state, filename.clone(), file.as_slice()).await?;

    info!("Indexing {}", filename);
    match index_file(state, &file, &filename, DocType::Offline) {
        Ok(_) => Ok(()),
        Err(error_index) => {
            info!(
                "Could not index file: {:?}. Deleting {} from s3 storage",
                error_index, filename
            );
            s3::delete_file(state, filename)
                .await
                .map_err(|error_delete| {
                    Custom(
                        Status::InternalServerError,
                        format!("{:?}\tAND\t{}", error_index, error_delete.1),
                    )
                })
        }
    }
}

// TODO: Check if the URL is already in the database
/// Index a document and add it to the database
///
/// The URL **must** be an encoded url such what `encodeURIComponent`
/// in Javascript results to.
///
/// # Errors
///
/// Errors might originate from the database, Diesel, or Rocket
#[post("/docs/url/<url>")]
pub async fn index_url(
    url: String,
    state: &State<ServerState>,
    _auth: UserSession<'_>,
) -> ApiResponse<()> {
    use url::form_urlencoded::parse;
    let url = parse(url.as_bytes())
        .map(|(k, v)| [k, v].concat())
        .collect();
    info!("Indexing URL {}", &url);
    info!("== Downloading {}", &url);
    let document = fetch_content(&url).await?;
    info!("== Downloaded {}", &url);
    index_file(state, &document, url.as_str(), DocType::Online)?;
    Ok(())
}

// Deleting from the database /////////////////////////////////////////////////

/// Delete the document `id`
///
/// # Errors
///
/// Errors might originate from the database, Diesel, or Rocket
#[delete("/docs/<id>")]
pub fn delete_document(
    id: &str,
    state: &State<ServerState>,
    _auth: UserSession<'_>,
) -> ApiResponse<()> {
    info!("Deleting document \"{}\"", id);
    let conn = &mut get_connector!(state);
    db::delete_document(conn, id)
        .map(|_| {
            info!("Deleted document \"{}\"", id);
        })
        .map_err(|e| api_error!(e.to_string()))
}

// Reading the database ///////////////////////////////////////////////////////

fn search_document_by_keyword(
    conn: &mut DbPool,
    query: &[String],
    spelling_suggestion: &[String],
    using_suggestion: &UseSpellingSuggestion,
) -> Result<Json<QueryResult>> {
    match using_suggestion {
        // If we are already using the spelling suggestion, return
        // what we have
        UseSpellingSuggestion::Yes => {
            let results = db::keywords_search(conn, spelling_suggestion)?;
            Ok(Json(QueryResult::new(
                results,
                Some(spelling_suggestion.join(" ")),
                using_suggestion,
            )))
        }
        // If we are not usin the spelling suggestion, only try
        UseSpellingSuggestion::No => {
            // If the results are not empty, or if the spelling
            // suggestion bears no difference with the initial query,
            // return what we have
            let results = db::keywords_search(conn, query)?;
            if !results.is_empty() || query == spelling_suggestion {
                Ok(Json(QueryResult::new(results, None, using_suggestion)))
            } else {
                // Otherwise, if the results were empty and the
                // initial query is different from the spelling
                // suggestion, try to search the database using it
                search_document_by_keyword(
                    conn,
                    query,
                    spelling_suggestion,
                    &UseSpellingSuggestion::Yes,
                )
            }
        }
    }
}

/// Search documents matching the keywords in `query`
///
/// This function also executes a spell check on the query. If the
/// function detects no results are found from the initial query, it
/// will try to find other results using the spell checked version of
/// the query. Whether the spell checked version of the query has been
/// used or not is specified in the [`QueryResult`] type returned.
///
/// # Errors
///
/// Errors might originate from the database, Diesel, or Rocket
///
/// [`QueryResult`]: ./struct.QueryResult.html
#[get("/search/<query>")]
pub fn search_query(
    query: &str,
    state: &State<ServerState>,
) -> ApiResponse<Json<QueryResult>> {
    use crate::spelling::correct;
    // Filter out empty queries
    info!("Query \"{}\"", query);
    if query.is_empty() {
        return Ok(Json(QueryResult::default()));
    }

    let conn = &mut get_connector!(state);

    // Normalize query
    let glaff = &state.glaff;
    let query_vec = query
        .split_whitespace()
        .map(|s| kwparser::get_lemma_from_glaff(s.to_lowercase(), glaff))
        .collect::<Vec<String>>();

    // Spellcheck query
    debug!("Normalized query_vec: {:?}", query_vec);
    let spelling_suggestion = query_vec
        .iter()
        .map(|s| {
            kwparser::get_lemma_from_glaff(
                correct(s.to_string(), &state.dictionary),
                glaff,
            )
        })
        .collect::<Vec<String>>();

    // Execute the query
    debug!("Suggested query: {:?}", spelling_suggestion);
    search_document_by_keyword(
        conn,
        &query_vec,
        &spelling_suggestion,
        &UseSpellingSuggestion::No,
    )
    .map_err(|e| Custom(Status::InternalServerError, e.to_string()))
}

/// List indexed documents
///
/// # Errors
///
/// Errors might originate from the database, Diesel, or Rocket
#[get("/docs")]
pub fn list_docs(
    state: &State<ServerState>,
) -> ApiResponse<Json<Vec<Document>>> {
    info!("Listing documents");
    let conn = &mut get_connector!(state);
    json_val_or_error!(db::list_documents(conn))
}

/// List keywords associated with a document
///
/// # Errors
///
/// Errors might originate from the database, Diesel, or Rocket
#[get("/docs/<doc>/keywords")]
pub fn document_list_keywords(
    doc: &str,
    state: &State<ServerState>,
) -> ApiResponse<Json<Vec<RankedKeyword>>> {
    info!("Getting document \"{}\"", doc);
    let conn = &mut get_connector!(state);
    json_val_or_error!(db::doc_list_keywords(conn, doc))
}

// Utilities //////////////////////////////////////////////////////////////////
#[get("/spelling/<word>")]
#[must_use]
pub fn spelling_word(word: String, state: &State<ServerState>) -> String {
    crate::spelling::correct(word, &state.dictionary)
}
