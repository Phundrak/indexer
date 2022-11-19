#![warn(clippy::style, clippy::pedantic)]
#![allow(clippy::no_effect_underscore_binding)]

#[macro_use]
extern crate rocket;

use std::error::Error;

use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use std::path::PathBuf;
use structopt::StructOpt;

use rocket::http::Method;
use rocket_cors::{AllowedHeaders, AllowedOrigins, AllOrSome, Origins, Cors};

mod db;
mod kwparser;
mod server;
mod fileparser;

#[derive(StructOpt, Debug)]
#[structopt(name = "indexer")]
struct Opt {
    /// Path to a list of stop words to ignore
    #[structopt(short = "s", long, parse(from_os_str))]
    stop_words: PathBuf,

    /// Path to the CSV version of the GLÀFF (optional)
    #[structopt(short = "g", long, parse(from_os_str))]
    glaff: Option<PathBuf>,
}

pub fn setup_loggin() {
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::DEBUG)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default subscriber failed");
}

fn make_cors(allowed_origins: AllOrSome<Origins>) -> Result<Cors, rocket_cors::Error> {
    rocket_cors::CorsOptions {
        allowed_origins,
        allowed_methods: vec![Method::Get, Method::Post, Method::Delete]
            .into_iter()
            .map(From::from)
            .collect(),
        allowed_headers: AllowedHeaders::some(&["Authorization", "Accept"]),
        allow_credentials: true,
        ..Default::default()
    }
    .to_cors()
}

#[rocket::main]
async fn main() -> Result<(), Box<dyn Error>> {
    setup_loggin();

    let opt = Opt::from_args();

    info!("Reading stopwords");
    let stopwords = kwparser::get_stopwords(opt.stop_words);
    info!("Reading GLÀFF");
    let glaff = kwparser::parse_glaff(opt.glaff);

    let allowed_origins = AllowedOrigins::some_regex(&[".*"]);
    let cors = make_cors(allowed_origins)?;

    info!("Launching server");
    #[allow(clippy::let_underscore_drop)]
    let _ = rocket::build()
        .mount(
            "/",
            routes![
                // POST
                server::index_url,      // /doc?url=:url
                // DELETE
                server::delete_document, // /doc?id=:id
                // GET
                server::search_query,           // /search?query=:query
                server::list_docs,              // /doc
                server::document_list_keywords, // /doc/:id
            ],
        )
        .attach(cors)
        .manage(server::ServerState {
            pool: db::get_connection_pool(),
            stopwords,
            glaff,
        })
        .launch()
        .await?;
    Ok(())
}
