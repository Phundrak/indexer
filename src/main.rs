use std::collections::{HashMap, HashSet};
use std::fs::{self, read_to_string};
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "basic")]
struct Opt {
    #[structopt(name = "DIRECTORY", parse(from_os_str))]
    directory: PathBuf,
}

fn get_files_in_dir(dir: PathBuf) -> Vec<PathBuf> {
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

fn get_keywords_from_file(
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

fn main() {
    let opt = Opt::from_args();
    let files = get_files_in_dir(opt.directory);
    let mut keywords: HashMap<String, HashSet<PathBuf>> = HashMap::new();

    for file in files {
        get_keywords_from_file(&file, &mut keywords);
    }
}
