use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use color_eyre::eyre::Result;
use indexer::kwparser::Glaff;
use rocket::serde::Deserialize;
use structopt::StructOpt;
use tracing::info;

#[derive(StructOpt)]
#[structopt(name = "glaff_compiler")]
struct Opt {
    /// Output file
    #[structopt(short = "o", long, parse(from_os_str))]
    output: PathBuf,

    /// Path to the GLÀFF
    #[structopt(name = "FILE", parse(from_os_str))]
    file: PathBuf,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
struct GlaffRecord<'a> {
    word: &'a str,
    _grace: &'a str,
    lemme: &'a str,
    _ipa: &'a str,
    _sampa: &'a str,
    _frq1: f32,
    _frq2: f32,
    _frq3: f32,
    _frq4: f32,
    _frq5: f32,
    _frq6: f32,
    _frq7: f32,
    _frq8: f32,
    _frq9: f32,
    _frq10: f32,
    _frq11: f32,
    _frq12: f32,
}

/// Parse the GLÀFF
///
/// Results in a `HashMap` containing on the first hand pretty much
/// all words in the French language, and on the other hand its
/// canonical form.
///
/// If `path` is `None`, return nothing (useful when not dealing with
/// French text)
///
/// # Panics
///
/// The program may panic if it fails to correctly parse the GLÀFF. If
/// it does, verify if your version is not corrupted.
pub fn parse_glaff(file: PathBuf) -> Result<Glaff> {
    let mut reader = csv::ReaderBuilder::new()
        .delimiter(b'|')
        .has_headers(false)
        .from_path(file)
        .unwrap();
    let mut lemme: Glaff = HashMap::new();
    for record in reader.records() {
        let row = record?;
        let row: GlaffRecord = row.deserialize(None)?;
        lemme.insert(row.word.to_string(), row.lemme.to_string());
    }
    Ok(lemme)
}

fn main() -> Result<()> {
    indexer::setup_logging();
    let opt = Opt::from_args();
    info!("Reading the GLÀFF");
    let glaff = parse_glaff(opt.file)?;
    info!("Converting GLÀFF to binary");
    let glaff_bin = bincode::serialize(&glaff)?;
    info!("Writing to {}", opt.output.display());
    let mut file = File::create(opt.output)?;
    file.write_all(glaff_bin.as_ref())?;
    Ok(())
}
