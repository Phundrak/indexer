use std::fmt::Debug;

pub mod html;
pub mod pdf;

#[derive(Debug)]
pub struct FileParsingError(String);

impl FileParsingError {
    pub fn new<E>(error: E) -> Self where E: Debug {
        Self(format!("{:?}", error))
    }
}

#[derive(Debug)]
pub struct ParsedDocument {
    pub title: String,
    pub keywords: Vec<String>,
    pub content: Vec<String>,
    pub description: String,
}

pub type ParsedFile = (String, Vec<String>, String, Option<String>);
pub type ParsingResult = Result<ParsedFile, FileParsingError>;

pub fn get_content(
    doc: &[u8],
    stop_words: &[String],
    glaff: &Option<crate::kwparser::Glaff>,
) -> Result<ParsedDocument, FileParsingError> {
    use crate::kwparser::get_keywords_from_text;

    let content = match infer::get(doc) {
        None => Err(FileParsingError("No mime type detected".into())),
        Some(mime) => match mime.mime_type() {
            "application/pdf" => pdf::parse(doc),
            "text/html" => html::parse(doc),
            mime => Err(FileParsingError(format!(
                "Mime type {} not supported",
                mime
            ))),
        },
    };
    let content = match content {
        Ok(val) => val,
        Err(e) => {
            return Err(e);
        }
    };
    let keywords = get_keywords_from_text(&content.2, stop_words, glaff);
    Ok(ParsedDocument {
        title: content.0,
        keywords: content.1,
        content: keywords,
        description: content.3.unwrap_or_else(|| content.2[..120].replace('\n', " ")),
    })
}
