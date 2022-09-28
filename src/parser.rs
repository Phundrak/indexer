use std::collections::{HashMap, HashSet};
use std::fs::{self, read_to_string};
use std::path::PathBuf;

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

pub fn get_keywords_from_file(
    file: &PathBuf,
    keywords: &mut HashMap<String, HashSet<PathBuf>>,
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
            if e.len() > 2 {
                Some(e.to_lowercase())
            } else {
                None
            }
        })
        .collect();
    println!("{:?}", words);
    for word in words {
        if !keywords.contains_key(&word) {
            keywords.insert(word.clone(), HashSet::new());
        }
        keywords.get_mut(&word).unwrap().insert(file.to_path_buf());
    }
    println!("Keywords: {:?}", keywords);
}
