use color_eyre::eyre::Result;
use indexer::kwparser::{get_keywords_from_text, get_stopwords};
use indexer::spelling::Dictionary;
use std::fs::File;
use std::io::Write;
use std::{collections::HashMap, fs::read_to_string, path::PathBuf};
use structopt::StructOpt;

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

fn train(files: &[PathBuf], stop_words: &[String]) -> HashMap<String, usize> {
    let mut keywords: HashMap<String, usize> = HashMap::new();
    files
        .iter()
        .map(read_to_string)
        .flat_map(|s| get_keywords_from_text(&s.unwrap(), stop_words, &None))
        .for_each(|word| {
            keywords
                .entry(word)
                .and_modify(|value| *value += 1)
                .or_insert(1);
        });
    keywords
}

fn main() -> Result<()> {
    indexer::setup_logging();
    color_eyre::install()?;
    let opt = Opt::from_args();
    let stop_words = get_stopwords(opt.stop_words);
    let keywords = train(&opt.files, &stop_words);
    let dictionary = Dictionary {
        n: keywords.iter().fold(0, |acc, (_, value)| acc + value),
        words: keywords,
        edits: Vec::new(),
    };
    let dictionary_bin = bincode::serialize(&dictionary)?;
    let mut file = File::create(opt.output)?;
    file.write_all(dictionary_bin.as_ref())?;
    Ok(())
}
