use std::collections::HashMap;

use color_eyre::eyre::Result;
use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool, PooledConnection};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::State;
use tracing::{debug, info};

use crate::db::{self, models::Document};
use crate::fileparser::get_content;
use crate::kwparser;
use crate::spelling::Dictionary;

type DbPool = PooledConnection<ConnectionManager<PgConnection>>;

#[allow(clippy::module_name_repetitions)]
pub struct ServerState {
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub stopwords: Vec<String>,
    pub glaff: Option<HashMap<String, String>>,
    pub dictionary: Option<Dictionary>,
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

// TODO: Check if the URL is already in the database
/// Index a document and add it to the database
///
/// # Errors
///
/// Errors might originate from the database, Diesel, or Rocket
#[post("/doc?<url>")]
pub async fn index_url(
    url: String,
    state: &State<ServerState>,
) -> ApiResponse<()> {
    info!("Indexing {}", &url);
    info!("== Downloading {}", &url);
    let document = fetch_content(&url).await?;
    info!("== Downloaded {}", &url);
    let stop_words = &state.stopwords;
    let glaff = &state.glaff;
    let content = get_content(&document, stop_words, glaff)
        .map_err(|e| Custom(Status::NotAcceptable, format!("{:?}", e)))?;
    debug!("{:?}", content);
    info!("== Downloaded {}", &url);
    let conn = &mut state.pool.get().map_err(|e| {
        api_error!(format!("Failed to connect to the database: {}", e))
    })?;
    info!("== Inserting {} in database", &url);

    let doc = Document {
        title: content.title.clone(),
        name: url.clone(),
        doctype: db::models::DocType::Online,
        description: content.description.clone(),
    };
    db::add_document(conn, &doc, &content).map_err(|e| {
        Custom(
            Status::InternalServerError,
            format!("Failed to insert URL {} as a document: {}", url, e),
        )
    })?;
    info!("Indexed {}", &url);
    Ok(())
}

// Deleting from the database /////////////////////////////////////////////////

/// Delete the document `id`
///
/// # Errors
///
/// Errors might originate from the database, Diesel, or Rocket
#[delete("/doc?<id>")]
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
        },
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
#[get("/doc")]
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
#[get("/doc/<id>")]
pub fn document_list_keywords(
    id: &str,
    state: &State<ServerState>,
) -> ApiResponse<Json<Vec<RankedKeyword>>> {
    info!("Getting document \"{}\"", id);
    let conn = &mut get_connector!(state);
    json_val_or_error!(db::doc_list_keywords(conn, id))
}

// Utilities //////////////////////////////////////////////////////////////////
#[get("/spelling/<word>")]
#[must_use]
pub fn spelling_word(word: String, state: &State<ServerState>) -> String {
    crate::spelling::correct(word, &state.dictionary)
}
