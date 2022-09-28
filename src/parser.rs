use std::collections::{HashMap, HashSet};
use std::fmt::{self, Display};
use std::fs::{self, read_to_string};
use std::path::PathBuf;

use comfy_table::Table;

// type Indexer = HashMap<String, HashSet<PathBuf>>;
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

pub fn get_keywords_from_file(file: &PathBuf, keywords: &mut Indexer) {
    let content = read_to_string(file).unwrap();
    let words: Vec<String> = content
        .split(
            &[
                ' ', '(', ')', '*', ',', '.', '/', ';', '[', '\'', '\\', '\n',
                ']', '^', '_', '{', '}', ' ', '«', '»', '’', '…', ' ',
            ][..],
        )
        .filter_map(|e| {
            if e.len() > 2 {
                Some(e.to_lowercase())
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
