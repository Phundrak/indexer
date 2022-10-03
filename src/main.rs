use std::path::PathBuf;
use structopt::StructOpt;

mod parser;

use parser::Indexer;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(name = "DIRECTORY", parse(from_os_str))]
    directory: PathBuf,

    #[structopt(short = "s", long, parse(from_os_str))]
    stop_words: Option<PathBuf>,

    #[structopt(short = "g", long, parse(from_os_str))]
    glaff: Option<PathBuf>,
}

fn main() {
    let opt = Opt::from_args();
    let files = parser::get_files_in_dir(opt.directory);
    let stop_words = parser::get_stopwords(opt.stop_words);
    let _lemmes = parser::get_lemmes(opt.glaff);
    let mut keywords = Indexer::new();

    for file in files {
        parser::get_keywords_from_file(&file, &mut keywords, &stop_words);
    }

    println!("Keywords detected:");
    println!("{keywords}");
}
