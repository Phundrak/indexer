#[macro_use]
extern crate rocket;

use std::error::Error;

use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

use std::path::PathBuf;
use structopt::StructOpt;

use rocket::http::Method;
use rocket_cors::{AllowedHeaders, AllowedOrigins};

mod db;
mod parser;
mod server;

#[derive(StructOpt, Debug)]
#[structopt(name = "indexer")]
struct Opt {
    #[structopt(short = "s", long, parse(from_os_str))]
    stop_words: PathBuf,

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

#[rocket::main]
async fn main() -> Result<(), Box<dyn Error>> {
    setup_loggin();

    let opt = Opt::from_args();

    info!("Reading stopwords");
    let stopwords = parser::get_stopwords(opt.stop_words);
    info!("Reading GLÀFF");
    let glaff = parser::parse_glaff(opt.glaff);

    let allowed_origins = AllowedOrigins::some_regex(&[".*"]);
    let cors = rocket_cors::CorsOptions {
        allowed_origins,
        allowed_methods: vec![Method::Get, Method::Post, Method::Delete]
            .into_iter()
            .map(From::from)
            .collect(),
        allowed_headers: AllowedHeaders::some(&["Authorization", "Accept"]),
        allow_credentials: true,
        ..Default::default()
    }
    .to_cors()?;

    info!("Launching server");
    let _ = rocket::build()
        .mount(
            "/",
            routes![
                server::search_keyword, // /keyword?keyword=:keyword GET
                server::search_multiple_words, // /search?query=:query GET
                server::list_docs,      // /doc GET
                server::index_url,      // /doc?url=:url             POST
                server::document_list_keywords, // /doc?doc=:id              GET
                server::delete_document, // /doc?id=:id                      DELETE
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
