use std::collections::HashMap;

use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::State;
use tracing::{debug, info};

use crate::db::{self, models::Document};
use crate::fileparser::get_content;
use crate::kwparser;
use crate::spelling::Dictionary;

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
}

impl QueryResult {
    #[must_use]
    pub fn new(
        results: Vec<RankedDoc>,
        initial_query: &str,
        query_suggestion: String,
    ) -> Self {
        Self {
            spelling_suggestion: if initial_query == query_suggestion {
                None
            } else {
                Some(query_suggestion)
            },
            results,
        }
    }
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
        match $result {
            Ok(val) => Ok(Json(val)),
            Err(e) => Err(Custom(Status::InternalServerError, e.to_string())),
        }
    };
}

pub type ApiResponse<T> = Result<T, Custom<String>>;

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
#[post("/doc/<url>")]
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
#[delete("/doc/<id>")]
pub fn delete_document(
    id: &str,
    state: &State<ServerState>,
) -> ApiResponse<()> {
    info!("Deleting document \"{}\"", id);
    let conn = &mut get_connector!(state);
    match db::delete_document(conn, id) {
        Ok(_) => {
            info!("Deleted document \"{}\"", id);
            Ok(())
        }
        Err(e) => Err(api_error!(e.to_string())),
    }
}

// Reading the database ///////////////////////////////////////////////////////

/// Search documents matching the keywords in `query`
///
/// # Errors
///
/// Errors might originate from the database, Diesel, or Rocket
#[get("/search/<query>")]
pub fn search_query(
    query: &str,
    state: &State<ServerState>,
) -> ApiResponse<Json<QueryResult>> {
    use crate::spelling::correct;
    info!("Query \"{}\"", query);
    if query.is_empty() {
        return Ok(Json(QueryResult::default()));
    }
    let conn = &mut get_connector!(state);
    let glaff = &state.glaff;
    let query_vec = query
        .split_whitespace()
        .map(|s| kwparser::get_lemma_from_glaff(s.to_lowercase(), glaff))
        .collect::<Vec<String>>();
    let query_suggestion = query_vec
        .iter()
        .map(|s| correct(s.to_string(), &state.dictionary))
        .collect::<Vec<String>>()
        .concat();
    debug!("Normalized query_vec: {:?}", query_vec);
    debug!("Suggested query: {}", query_suggestion);
    db::keywords_search(conn, &query_vec)
        .map(|results| Json(QueryResult::new(results, query, query_suggestion)))
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
