#[macro_use]
extern crate rocket;

mod db;
mod parser;

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
    rocket::build().mount("/", routes![]).manage(db::StateMgr {
        connection: db::establish_connection(),
        stop_words: parser::get_stopwords(opt.stop_words),
        glaff: parser::get_lemmes(opt.glaff),
    })
}
