#[macro_use]
extern crate rocket;

use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod db;
mod parser;
mod server;

use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "indexer")]
struct Opt {
    #[structopt(short = "s", long, parse(from_os_str))]
    stop_words: PathBuf,

    #[structopt(short = "g", long, parse(from_os_str))]
    glaff: Option<PathBuf>,
}

#[launch]
fn rocket() -> _ {
    let opt = Opt::from_args();
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::TRACE)
        .finish();
    tracing::subscriber::set_global_default(subscriber)
        .expect("Setting default subscriber failed");
    info!("Reading stopwords");
    let stopwords = parser::get_stopwords(opt.stop_words);
    info!("Reading GLÀFF");
    let glaff = parser::parse_glaff(opt.glaff);
    info!("Launching server");
    rocket::build()
        .mount(
            "/",
            routes![
                server::index_url,              // /                 POST
                server::search_keyword,         // /keyword/:keyword GET
                server::document_list_keywords, // /document/:id     GET
                server::delete_document,        // /document         DELETE
            ],
        )
        .manage(server::ServerState {
            pool: db::get_connection_pool(),
            stopwords,
            glaff,
        })
}
