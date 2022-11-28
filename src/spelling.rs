use color_eyre::eyre::Result;
use rocket::serde::{Deserialize, Serialize};
use std::{collections::HashMap, path::PathBuf};

/// All characters used in the languages stored in the database
pub static ALPHABET: &str = "aàâbcdeéèëêfghiîïjklmnoôpqrstuûüvwxyÿz";

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
pub struct Dictionary {
    pub n: usize,
    pub words: HashMap<String, usize>,
    pub edits: Vec<String>,
}

/// Read a dictionary from a binary file
///
/// # Errors
///
/// If the function fails to read the file or if it fails to
/// deserialize its content, the error is returned to the caller of
/// `read_dictionary`.
///
/// # Panics
///
/// If a dictionary is provided but the file is not readable or not
/// deserializable, the program will panic.
pub fn read_dictionary(path: Option<PathBuf>) -> Result<Option<Dictionary>> {
    match path {
        None => Ok(None),
        Some(p) => {
            let data = std::fs::read(p)?;
            Ok(Some(bincode::deserialize(&data)?))
        }
    }
}

#[must_use]
pub fn correct(word: String, dictionary: &Option<Dictionary>) -> String {
    match dictionary {
        None => word,
        Some(dictionary) => {
            if dictionary.words.contains_key(&word) {
                return word;
            };

            let mut candidates: HashMap<usize, String> = HashMap::new();
            let edits_w = edits(&word);

            // Try to find an edit of the current word in the dictionary
            for edit in &edits_w {
                if let Some(score) = dictionary.words.get(edit) {
                    candidates.insert(*score, edit.to_string());
                }
            }
            // Return the most likely word among possible variations
            if let Some(c) = candidates.iter().max_by_key(|&e| e.0) {
                return c.1.to_string();
            }

            // Try to find the correct word in the edits_w of the edits_w
            for edit in &edits_w {
                for w in edits(edit) {
                    if let Some(score) = dictionary.words.get(&w) {
                        candidates.insert(*score, w);
                    }
                }
            }
            // Again, try to retu
            if let Some(c) = candidates.iter().max_by_key(|&e| e.0) {
                return c.1.to_string();
            }

            // Can’t find anything, return the word itself
            word
        }
    }
}

/// Create possible alterations of a word.
///
/// Create different kinds of alterations to `word`. It generates
/// possible character deletion, transposition, alteration or
/// insertion. The latter two alterations use the letters listed in
/// [`ALPHABET`].
///
/// [`ALPHABET`]: ./static.ALPHABET.html
#[must_use]
pub fn edits(word: &str) -> Vec<String> {
    let mut results: Vec<String> = Vec::new();
    let word_vec = word.chars().collect::<Vec<_>>();
    for i in 0..word_vec.len() {
        let (first, last) = word_vec.split_at(i);
        // deletion
        results.push([first, &last[1..]].concat().into_iter().collect());
        // transposition
        if i < word_vec.len() - 1 {
            results.push(
                [first, &last[1..2], &last[..1], &last[2..]]
                    .concat()
                    .into_iter()
                    .collect(),
            );
        }
        for c in ALPHABET.chars() {
            // alteration
            results
                .push([first, &[c], &last[1..]].concat().into_iter().collect());
            // insertion
            results.push([first, &[c], last].concat().into_iter().collect());
        }
    }
    results
}
