use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;

static PUNCTUATION: &[char] = &[
    ' ', '(', ')', '*', ',', '.', '/', ';', '[', '\'', '\\', '\n', ']', '^',
    '_', '{', '}', ' ', '«', '»', '’', '…', ' ',
];

pub fn get_stopwords(path: PathBuf) -> Vec<String> {
    let content = read_to_string(path).unwrap();
    let words: Vec<String> =
        content.split('\n').map(|e| e.to_string()).collect();
    words
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

fn is_stopword(word: &String, stop_words: &Vec<String>) -> bool {
    stop_words.contains(word)
}

fn is_short_word(word: &String) -> bool {
    word.len() <= 2
}

fn get_lemme(word: String, lemmes: &Option<HashMap<String, String>>) -> String {
    match lemmes {
        None => word,
        Some(collection) => match collection.get(&word) {
            Some(lemme) => lemme.clone(),
            None => word,
        },
    }
}

pub fn get_keywords_from_text(
    text: String,
    stop_words: &Vec<String>,
    lemmes: &Option<HashMap<String, String>>,
) -> Vec<String> {
    let words: Vec<String> = text
        .split(PUNCTUATION)
        .filter_map(|e| {
            let word = get_lemme(e.to_lowercase(), lemmes);
            if !is_short_word(&word) && !is_stopword(&word, stop_words) {
                Some(word)
            } else {
                None
            }
        })
        .collect();
    words
}
