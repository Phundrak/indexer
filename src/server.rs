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

#[derive(Copy, Clone)]
pub struct ContentLength(usize);

#[derive(Debug)]
pub enum ContentLengthError {
    Missing,
    Invalid,
}

use rocket::request::{FromRequest, Outcome, Request};
#[rocket::async_trait]
impl<'r> FromRequest<'r> for ContentLength {
    type Error = ContentLengthError;

    async fn from_request(
        request: &'r Request<'_>,
    ) -> Outcome<Self, Self::Error> {
        match request.headers().get_one("Content-Length") {
            None => Outcome::Failure((
                Status::BadRequest,
                ContentLengthError::Missing,
            )),
            Some(key) => match key.to_string().parse::<usize>() {
                Ok(val) => Outcome::Success(ContentLength(val)),
                Err(_) => Outcome::Failure((
                    Status::BadRequest,
                    ContentLengthError::Invalid,
                )),
            },
        }
    }
}

type DbPool = PooledConnection<ConnectionManager<PgConnection>>;

#[allow(clippy::module_name_repetitions)]
pub struct ServerState {
    pub dictionary: Option<Dictionary>,
    pub glaff: Option<HashMap<String, String>>,
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub stopwords: Vec<String>,
    pub s3_bucket: s3::Bucket,
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

/// Upload and index a document
///
/// # Errors
///
/// TODO: I’ll document that later
#[post("/docs/<filename>", data = "<file>")]
pub async fn index_upload(
    state: &State<ServerState>,
    mut file: TempFile<'_>,
    filename: String,
) -> ApiResponse<()> {
    use sha256::digest;
    file.move_copy_to("tmp/file")
        .await
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    let file = std::fs::read("tmp/file")
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    std::fs::remove_file("tmp/file")
        .map_err(|e| Custom(Status::InternalServerError, e.to_string()))?;
    info!("Deleted temporary file");

    let id = digest(&file as &[u8]);
    let filename = format!("{}-{}", id, filename);

    info!("Uploading file {}", filename);
    state
        .s3_bucket
        .put_object(format!("/{}", filename.clone()), file.as_slice())
        .await
        .map_err(|e| {
            info!("Failed to upload file: {}", e);
            Custom(
                Status::InternalServerError,
                format!("Failed to upload file: {}", e),
            )
        })?;

    info!("Indexing {}", filename);
    match index_file(state, &file, &filename, DocType::Offline) {
        Ok(_) => Ok(()),
        Err(e1) => match state
            .s3_bucket
            .delete_object(format!("/{}", filename))
            .await
        {
            Ok(_) => Err(e1),
            Err(e2) => Err(Custom(
                Status::InternalServerError,
                format!("{:?}\nAlso failed to remove remote file: {}", e1, e2),
            )),
        },
    }
}

// TODO: Check if the URL is already in the database
/// Index a document and add it to the database
///
/// # Errors
///
/// Errors might originate from the database, Diesel, or Rocket
#[post("/docs/url/<url>")]
pub async fn index_url(
    url: String,
    state: &State<ServerState>,
) -> ApiResponse<()> {
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
        .map(|s| correct(s.to_string(), &state.dictionary))
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
pub fn list_docs(state: &State<ServerState>) -> ApiResponse<Json<Vec<String>>> {
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
