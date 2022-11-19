use indexer::kwparser::{get_keywords_from_text, get_stopwords};
use rayon::prelude::{IntoParallelRefIterator, ParallelIterator};
use rocket::serde::Serialize;
use tracing::debug;
use std::{collections::HashMap, fs::read_to_string, path::PathBuf};
use std::io::Write;
use std::fs::File;
use structopt::StructOpt;
use color_eyre::eyre::Result;

#[derive(StructOpt, Debug)]
#[structopt(name = "spelltrainer")]
struct Opt {
    /// List of stop words to ignore
    #[structopt(short = "s", long, parse(from_os_str))]
    stop_words: PathBuf,

    /// Output path of the dictionary
    #[structopt(short = "o", long, parse(from_os_str))]
    output: PathBuf,

    /// Corpus files to process
    #[structopt(name = "FILE", parse(from_os_str))]
    files: Vec<PathBuf>,
}

#[derive(Serialize, Debug)]
#[serde(crate = "rocket::serde")]
struct Dictionary {
    n: usize,
    words: HashMap<String, usize>,
}

fn main() -> Result<()> {
    indexer::setup_logging();
    color_eyre::install()?;
    let opt = Opt::from_args();
    let stop_words = get_stopwords(opt.stop_words);
    let corpus_dir = opt.files;
    let raw_keywords = corpus_dir
        .par_iter()
        .map(read_to_string)
        .map(|s| get_keywords_from_text(&s.unwrap(), &stop_words, &None))
        .flatten()
        .collect::<Vec<String>>();
    let mut keywords: HashMap<String, usize> = HashMap::new();
    for word in raw_keywords {
        keywords.entry(word).and_modify(|k| *k += 1).or_insert(1);
    }
    let keywords = Dictionary {
        n: keywords.iter().fold(0, |acc, (_, value)| acc + value),
        words: keywords,
    };
    let keywords = bincode::serialize(&keywords)?;
    let mut file = File::create(opt.output)?;
    file.write_all(keywords.as_ref())?;
    Ok(())
}
