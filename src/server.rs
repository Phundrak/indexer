use diesel::pg::PgConnection;
use diesel::r2d2::{ConnectionManager, Pool};
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket::serde::json::Json;
use rocket::State;
use scraper::{Html, Selector};
use tracing::info;

use crate::db;
use crate::parser::Glaff;
pub struct ServerState {
    pub pool: Pool<ConnectionManager<PgConnection>>,
    pub stopwords: Vec<String>,
    pub glaff: Option<Glaff>,
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
    let keywords = crate::parser::get_keywords_from_text(
        text,
        &state.stopwords,
        &state.glaff,
    );
    for keyword in keywords {
        ok_or_err!(
            db::insert_word(conn, &keyword, url),
            "{}: Could not insert keyword \"{}\"",
            keyword
        )
    }
    Ok(())
}

#[post("/url?<url>")]
pub async fn index_url(
    url: String,
    state: &State<ServerState>,
) -> ApiResponse<()> {
    info!("Indexing {}", &url);
    let body = match reqwest::get(&url).await {
        Ok(val) => match val.text().await {
            Ok(val) => val,
            Err(e) => {
                return Err(Custom(Status::InternalServerError, e.to_string()))
            }
        },
        Err(e) => {
            return Err(Custom(Status::InternalServerError, e.to_string()))
        }
    };
    info!("Downloaded {}", &url);
    let document = Html::parse_document(&body);
    let selector = Selector::parse("meta[name=keywords]").unwrap();
    let conn = &mut get_connector!(state);
    ok_or_err!(
        db::add_document(conn, &url),
        "{}: Failed to insert URL {} as a document",
        &url
    );
    for element in document.select(&selector) {
        parse_and_insert(element.inner_html(), &url, conn, state)?;
    }
    info!("Parsed keywords in {}", &url);
    let selector = Selector::parse("body").unwrap();
    for element in document.select(&selector) {
        let text = html2text::from_read(
            element.inner_html().as_bytes(),
            element.inner_html().len(),
        );
        parse_and_insert(text, &url, conn, state)?;
    }
    info!("Parsed {}", &url);
    Ok(())
}

#[get("/keyword?<keyword>")]
pub fn search_keyword(
    keyword: String,
    state: &State<ServerState>,
) -> ApiResponse<Json<Vec<String>>> {
    let conn = &mut get_connector!(state);
    json_val_or_error!(db::keyword_list_docs(conn, &keyword))
}

#[get("/doc?<doc>")]
pub fn document_list_keywords(
    doc: String,
    state: &State<ServerState>,
) -> ApiResponse<Json<Vec<String>>> {
    let conn = &mut get_connector!(state);
    json_val_or_error!(db::doc_list_keywords(conn, &doc))
}

#[delete("/doc?<id>")]
pub fn delete_document(
    id: String,
    state: &State<ServerState>,
) -> ApiResponse<()> {
    let conn = &mut get_connector!(state);
    match db::delete_document(conn, &id) {
        Ok(_) => Ok(()),
        Err(e) => api_error!(e.to_string()),
    }
}
