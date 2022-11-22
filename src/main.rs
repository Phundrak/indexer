#![warn(clippy::style, clippy::pedantic)]
#![allow(clippy::no_effect_underscore_binding)]

#[macro_use]
extern crate rocket;

use tracing::info;
use color_eyre::eyre::Result;

use std::path::PathBuf;
use structopt::StructOpt;

use rocket::http::Method;
use rocket_cors::{AllOrSome, AllowedHeaders, AllowedOrigins, Cors, Origins};

mod db;
mod fileparser;
mod kwparser;
mod server;
mod spelling;

#[derive(StructOpt, Debug)]
#[structopt(name = "indexer")]
struct Opt {
    /// Path to the stop word list
    #[structopt(short = "s", long, parse(from_os_str))]
    stop_words: PathBuf,

    /// Path to the binary version of the GLÀFF (optional)
    #[structopt(short = "g", long, parse(from_os_str))]
    glaff: Option<PathBuf>,

    /// Path to the binary version of the dictionary (optional)
    #[structopt(short = "d", long, parse(from_os_str))]
    dictionary: Option<PathBuf>,
}

fn make_cors(
    allowed_origins: AllOrSome<Origins>,
) -> Result<Cors, rocket_cors::Error> {
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
async fn main() -> Result<()> {
    color_eyre::install()?;
    indexer::setup_logging();

    let opt = Opt::from_args();

    info!("Reading stopwords");
    let stopwords = kwparser::get_stopwords(opt.stop_words);
    info!("Reading GLÀFF");
    let glaff = kwparser::read_glaff(opt.glaff);
    info!("Reading dictionary");
    let dictionary = spelling::read_dictionary(opt.dictionary)?;

    let allowed_origins = AllowedOrigins::some_regex(&[".*"]);
    let cors = make_cors(allowed_origins)?;

    info!("Launching server");
    #[allow(clippy::let_underscore_drop)]
    let _ = rocket::build()
        .mount(
            "/",
            routes![
                // POST
                server::index_url, // /doc/:url
                // DELETE
                server::delete_document, // /doc/:id
                // GET
                server::search_query, // /searchy/:query
                server::list_docs,    // /doc
                server::document_list_keywords, // /doc/:id
                server::spelling_word, // /spelling/:word
            ],
        )
        .attach(cors)
        .manage(server::ServerState {
            pool: db::get_connection_pool(),
            stopwords,
            glaff,
            dictionary,
        })
        .launch()
        .await?;
    Ok(())
}
