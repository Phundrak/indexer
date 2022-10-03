use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display};
use std::fs::{self, read_to_string};
use std::path::PathBuf;

use comfy_table::Table;

#[derive(Debug)]
pub struct Indexer(pub HashMap<String, HashSet<PathBuf>>);

impl Indexer {
    pub fn new() -> Indexer {
        Indexer(HashMap::new())
    }
}

impl Default for Indexer {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for Indexer {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut table = Table::new();
        table.set_header(vec!["keyword", "files"]);
        for keyword in &self.0 {
            table.add_row(vec![
                keyword.0,
                &keyword
                    .1
                    .iter()
                    .map(|e| format!("{:?}", e))
                    .collect::<Vec<String>>()
                    .join(", "),
            ]);
        }
        write!(f, "{table}")
    }
}

pub fn get_files_in_dir(dir: PathBuf) -> Vec<PathBuf> {
    let files: Vec<PathBuf> = fs::read_dir(dir)
        .unwrap()
        .filter_map(|entry| {
            let path = entry.unwrap().path();
            if !path.is_dir() {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    files
}

pub fn get_stopwords(path: Option<PathBuf>) -> Option<Vec<String>> {
    match path {
        Some(file) => {
            let content = read_to_string(file).unwrap();
            let words: Vec<String> =
                content.split('\n').map(|e| e.to_string()).collect();
            Some(words)
        }
        None => None,
    }
}

pub fn get_lemmes(path: Option<PathBuf>) -> Option<HashMap<String, String>> {
    match path {
        None => None,
        Some(file) => {
            let mut reader = csv::ReaderBuilder::new()
                .delimiter(b'|')
                .has_headers(false)
                .from_path(file)
                .unwrap();
            let mut lemme: HashMap<String, String> = HashMap::new();
            for result in reader.records() {
                let record = result.unwrap();
                lemme.insert(
                    record.get(0).unwrap().to_string(),
                    record.get(2).unwrap().to_string(),
                );
            }
            Some(lemme)
        }
    }
}

fn is_stopword(word: &String, stop_words: &Option<Vec<String>>) -> bool {
    match stop_words {
        Some(words) => words.contains(word),
        None => false,
    }
}

fn is_short_word(word: &String) -> bool {
    word.len() <= 2
}

fn get_lemme(word: String, lemmes: &Option<HashMap<String, String>>) -> String {
    match lemmes {
        None => word,
        Some(collection) => {
            match collection.get(&word) {
                Some(lemme) => lemme.clone(),
                None => word
            }
        }
    }
}

pub fn get_keywords_from_file(
    file: &PathBuf,
    keywords: &mut Indexer,
    stop_words: &Option<Vec<String>>,
    lemmes: &Option<HashMap<String, String>>
) {
    let content = read_to_string(file).unwrap();
    let words: Vec<String> = content
        .split(
            &[
                ' ', '(', ')', '*', ',', '.', '/', ';', '[', '\'', '\\', '\n',
                ']', '^', '_', '{', '}', ' ', '«', '»', '’', '…', ' ',
            ][..],
        )
        .filter_map(|e| {
            let word = get_lemme(e.to_lowercase(), lemmes);
            if !is_short_word(&word) && !is_stopword(&word, stop_words) {
                Some(word)
            } else {
                None
            }
        })
        .collect();
    for word in words {
        if !keywords.0.contains_key(&word) {
            keywords.0.insert(word.clone(), HashSet::new());
        }
        keywords
            .0
            .get_mut(&word)
            .unwrap()
            .insert(file.to_path_buf());
    }
}
