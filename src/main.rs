use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use structopt::StructOpt;

mod parser;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(name = "DIRECTORY", parse(from_os_str))]
    directory: PathBuf,
}

fn main() {
    let opt = Opt::from_args();
    let files = parser::get_files_in_dir(opt.directory);
    let mut keywords: HashMap<String, HashSet<PathBuf>> = HashMap::new();

    for file in files {
        parser::get_keywords_from_file(&file, &mut keywords);
    }
}
