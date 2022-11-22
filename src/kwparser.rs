use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::PathBuf;

use rayon::prelude::*;

/// Get list of stopwords from a file.
///
/// The file pointed at by `path` must contain one stopword per line.
///
/// # Panics
///
/// If the program cannot read the file containing stop words, the
/// program will panic.
#[must_use]
pub fn get_stopwords(path: PathBuf) -> Vec<String> {
    let content = read_to_string(path).unwrap();
    let words: Vec<String> = content
        .split('\n')
        .map(std::string::ToString::to_string)
        .collect();
    words
}

pub type Glaff = HashMap<String, String>;

/// Read the GLÀFF in its binary version
///
/// # Panics
///
/// The program may panic if `path` is not readable or if the
/// deserialization fails.
#[must_use]
pub fn read_glaff(path: Option<PathBuf>) -> Option<Glaff> {
    match path {
        None => None,
        Some(p) => {
            let data = std::fs::read(p).unwrap();
            Some(bincode::deserialize(&data).unwrap())
        }
    }
}

/// Get a lemma from the GLAFF
#[must_use]
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

pub fn split_keywords<T>(keywords: &T) -> Vec<String>
where
    T: ToString,
{
    keywords
        .to_string()
        .split(&[',', ' '])
        .filter(|s| !s.is_empty())
        .map(std::string::ToString::to_string)
        .collect()
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

#[must_use]
#[allow(clippy::implicit_hasher)]
pub fn get_keywords_from_text(
    text: &str,
    stop_words: &[String],
    lemmes: &Option<Glaff>,
) -> Vec<String> {
    text.split(|c| !char::is_alphabetic(c))
        .par_bridge()
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
