use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;

static PUNCTUATION: &[char] = &[
    ' ', '(', ')', '*', ',', '.', '/', ';', '[', '\'', '\\', '\n',
    ']', '^', '_', '{', '}', ' ', '«', '»', '’', '…', ' ', '|', '↑',
    '─', '┼', '*', '│', ':', '┴', '┬', '↓', '%', '→'
];

/// Get list of stopwords from a file.
///
/// The file pointed at by `path` must contain one stopword per line.
pub fn get_stopwords(path: PathBuf) -> Vec<String> {
    let content = read_to_string(path).unwrap();
    let words: Vec<String> =
        content.split('\n').map(|e| e.to_string()).collect();
    words
}

pub type Glaff = HashMap<String, String>;

/// Parse the GLÀFF
///
/// Results in a HashMap containing on the first hand pretty much all
/// words in the French language, and on the other hand its canonical
/// form.
///
/// If `path` is `None`, return nothing (useful when not dealing with
/// French text)
pub fn parse_glaff(path: Option<PathBuf>) -> Option<Glaff> {
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

/// Get a lemma from the GLAFF
pub fn get_lemma_from_glaff(word: String, glaff: &Option<Glaff>) -> String {
    match glaff {
        None => word,
        Some(collection) => match collection.get(&word) {
            Some(lemme) => lemme.clone(),
            None => word,
        },
    }
}

/// Determine if a word is a stopword
fn is_stopword(word: &String, stop_words: &[String]) -> bool {
    stop_words.contains(word)
}

/// Determine if a word is a short word
///
/// # Examples
///
/// ```
/// assert!(is_short_word("je"));
/// assert!(!(is_short_word("bonjour")));
/// ```
fn is_short_word(word: &String) -> bool {
    word.len() <= 2
}

pub fn get_keywords_from_text(
    text: String,
    stop_words: &[String],
    lemmes: &Option<HashMap<String, String>>,
) -> Vec<String> {
    text.split(PUNCTUATION)
        .filter_map(|e| {
            let word = get_lemma_from_glaff(e.to_lowercase(), lemmes);
            if !is_short_word(&word) && !is_stopword(&word, stop_words) {
                Some(word)
            } else {
                None
            }
        })
        .collect::<Vec<String>>()
}
