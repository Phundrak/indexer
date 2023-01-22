use poppler::Document;
use tracing::{debug, info};

use crate::fileparser::{FileParsingError, ParsingResult};

#[derive(Debug)]
struct PdfParsingError(String);

fn get_title(doc: &Document) -> Result<String, PdfParsingError> {
    info!("=== PDF: Parsing title");
    doc.title().map_or_else(
        || Err(PdfParsingError("Could not get title".into())),
        |title| Ok(title.into()),
    )
}

fn get_keywords(doc: &Document) -> Vec<String> {
    info!("=== PDF: Parsing keywords");
    let mut keywords = Vec::new();
    if let Some(pdf_keywords) = doc.keywords() {
        keywords = crate::kwparser::split_keywords(&pdf_keywords);
    }
    debug!("====== PDF: Keywords: {keywords:?}");
    keywords
}

fn get_body(doc: &Document) -> String {
    info!("=== PDF: Parsing body");
    let nbr_pages = doc.n_pages();
    let mut body = String::new();
    for i in 0..nbr_pages {
        if let Some(page) = doc.page(i) {
            if let Some(text) = page.text() {
                body += " ";
                body += text.as_str();
            }
        }
    }
    body
}

fn get_subject(doc: &Document) -> Option<String> {
    info!("=== PDF: Parsing subject");
    doc.subject().map(|e| e.to_string())
}

/// Parse a PDF file
///
/// Receive a PDF file’s content raw, extract from it its title,
/// keywords, subject, and text.
///
/// # Errors
///
/// If any error occurs when parsing the PDF, return it to the caller
/// function. For more information, see [`PdfParsingError`].
///
/// [`PdfParsingError`]: ./struct.PdfParsingError.html
pub fn parse(doc: &[u8]) -> ParsingResult {
    info!("== PDF: Parsing document");
    let doc = poppler::Document::from_data(doc, None).map_err(|e| {
        FileParsingError::new(format!("Failed to parse PDF: {e:?}"))
    })?;
    let title = get_title(&doc).map_err(FileParsingError::new)?;
    let keywords = get_keywords(&doc);
    let body = get_body(&doc);
    let subject = get_subject(&doc);
    Ok((title, keywords, body, subject))
}
