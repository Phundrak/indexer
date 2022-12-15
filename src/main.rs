#![warn(clippy::style, clippy::pedantic)]
#![allow(clippy::no_effect_underscore_binding)]

#[macro_use]
extern crate rocket;

use color_eyre::eyre::Result;
use tracing::info;

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
    /// Path to a list of stop words to ignore
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
    let pool = db::get_connection_pool();
    info!("Running database migrations");
    db::run_migrations(&mut pool.get()?)?;

    let s3_bucket = s3::Bucket::new(
        std::env::var("S3_BUCKET_ID")
            .expect("S3_BUCKET_ID must be set!")
            .as_str(),
        s3::region::Region::Custom {
            region: std::env::var("S3_REGION").expect("S3_REGION must be set!"),
            endpoint: std::env::var("S3_ENDPOINT")
                .expect("S3_ENDPOINT must be set!"),
        },
        s3::creds::Credentials::default()?,
    )?;

    info!("Launching server");
    #[allow(clippy::let_underscore_drop)]
    let _ = rocket::build()
        .mount(
            "/",
            routes![
                // POST
                server::index_url,    // /doc?url=:url
                server::index_upload, // /doc + binary file
                // DELETE
                server::delete_document, // /doc?id=:id
                // GET
                server::search_query, // /searchy?query=:query
                server::list_docs,    // /doc
                server::document_list_keywords, // /keywords?doc=:id
                server::spelling_word, // /spelling/:word
            ],
        )
        .attach(cors)
        .manage(server::ServerState {
            dictionary,
            glaff,
            pool,
            stopwords,
            s3_bucket,
        })
        .launch()
        .await?;
    Ok(())
}
