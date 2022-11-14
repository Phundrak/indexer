use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::{json::Json, Deserialize, Serialize};
use rocket::State;
use scraper::{Html, Selector};
use tracing::{debug, info};

use crate::db::{self, models::Document};
use crate::kwparser::{self, Glaff};

pub struct ServerState {
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub stopwords: Vec<String>,
    pub glaff: Option<Glaff>,
}

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct RankedDoc {
    pub doc: String,
    pub title: String,
    pub hits: i32,
}

macro_rules! api_error {
    ($message:expr) => {
        Err(Custom(Status::InternalServerError, $message))
    };
}

macro_rules! get_connector {
    ($db:expr) => {
        match $db.pool.get() {
            Ok(val) => val,
            Err(_) => {
                return api_error!(
                    "Failed to connect to the database".to_owned()
                );
            }
        }
    };
}

macro_rules! ok_or_err {
    ($dbrun:expr,$message:expr,$($args:expr),+) => {
        match $dbrun {
            Ok(_) => (),
            Err(e) => {
                return api_error!(format!($message, e, $($args),+));
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

fn parse_and_insert(
    text: String,
    url: &String,
    conn: &mut PgConnection,
    state: &State<ServerState>,
) -> ApiResponse<()> {
    let keywords =
        kwparser::get_keywords_from_text(text, &state.stopwords, &state.glaff);
    let document = match db::get_document(conn, url) {
        Ok(val) => val,
        Err(e) => {
            return api_error!(e.to_string());
        }
    };
    for keyword in keywords {
        ok_or_err!(
            db::insert_word(conn, &keyword, document.to_owned()),
            "{}: Could not insert keyword \"{}\"",
            keyword
        )
    }
    Ok(())
}

#[get("/search?<query>")]
pub fn search_query(
    query: String,
    state: &State<ServerState>,
) -> ApiResponse<Json<Vec<RankedDoc>>> {
    info!("Query \"{}\"", query);
    if query.is_empty() {
        return Ok(Json(Vec::new()));
    }
    let conn = &mut get_connector!(state);
    let glaff = &state.glaff;
    let query = query
        .split_whitespace()
        .map(|s| kwparser::get_lemma_from_glaff(s.to_lowercase(), glaff))
        .collect::<Vec<String>>();
    debug!("Normalized query: {:?}", query);
    json_val_or_error!(db::keywords_search(conn, &query))
}

async fn get_url(url: &String) -> ApiResponse<String> {
    match reqwest::get(url).await {
        Ok(val) => match val.text().await {
            Ok(val) => Ok(val),
            Err(e) => Err(Custom(Status::InternalServerError, e.to_string())),
        },
        Err(e) => Err(Custom(Status::InternalServerError, e.to_string())),
    }
}

fn parse_html_keywords(
    conn: &mut PgConnection,
    document: &Html,
    url: &String,
    state: &State<ServerState>,
) -> ApiResponse<()> {
    let selector_keywords = Selector::parse("meta[name=keywords]").unwrap();
    for element in document.select(&selector_keywords) {
        parse_and_insert(element.inner_html(), url, conn, state)?;
    }
    info!("== Parsed keywords in {}", &url);
    Ok(())
}

fn parse_html_title(document: &Html) -> ApiResponse<String> {
    info!("== Parsing title");
    let selector = match Selector::parse("title") {
        Ok(val) => val,
        Err(_) => {
            return api_error!("Failed to parse selector".to_string());
        }
    };
    if let Some(title) = document.select(&selector).next() {
        let inner = title.inner_html();
        Ok(html2text::from_read(inner.as_bytes(), inner.len())
            .trim()
            .into())
    } else {
        api_error!(format!("Could not find title"))
    }
}

fn parse_html_body(
    conn: &mut PgConnection,
    document: &Html,
    url: &String,
    state: &State<ServerState>,
) -> ApiResponse<()> {
    let selector_body = Selector::parse("body").unwrap();
    for element in document.select(&selector_body) {
        let text = html2text::from_read(
            element.inner_html().as_bytes(),
            element.inner_html().len(),
        );
        parse_and_insert(text, url, conn, state)?;
    }
    Ok(())
}

#[post("/doc?<url>")]
pub async fn index_url(
    url: String,
    state: &State<ServerState>,
) -> ApiResponse<()> {
    info!("Indexing {}", &url);
    info!("== Downloading {}", &url);
    let body = get_url(&url).await?;
    info!("== Downloaded {}", &url);
    let conn = &mut get_connector!(state);
    let document = Html::parse_document(&body);
    let title = parse_html_title(&document)?;
    info!("== Inserting {} in database", &url);
    ok_or_err!(
        db::add_document(
            conn,
            Document {
                name: url.to_owned(),
                title
            }
        ),
        "{}: Failed to insert URL {} as a document",
        &url
    );
    parse_html_keywords(conn, &document, &url, state)?;
    parse_html_body(conn, &document, &url, state)?;
    info!("Indexed {}", &url);
    Ok(())
}

#[get("/doc")]
pub fn list_docs(state: &State<ServerState>) -> ApiResponse<Json<Vec<String>>> {
    info!("Listing documents");
    let conn = &mut get_connector!(state);
    json_val_or_error!(db::list_documents(conn))
}

#[get("/doc?<doc>")]
pub fn document_list_keywords(
    doc: String,
    state: &State<ServerState>,
) -> ApiResponse<Json<Vec<String>>> {
    info!("Getting document \"{}\"", doc);
    let conn = &mut get_connector!(state);
    json_val_or_error!(db::doc_list_keywords(conn, &doc))
}

#[delete("/doc?<id>")]
pub fn delete_document(
    id: String,
    state: &State<ServerState>,
) -> ApiResponse<()> {
    info!("Deleting document \"{}\"", id);
    let conn = &mut get_connector!(state);
    match db::delete_document(conn, &id) {
        Ok(_) => {
            info!("Deleted document \"{}\"", id);
            Ok(())
        }
        Err(e) => api_error!(e.to_string()),
    }
}
