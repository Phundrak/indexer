use std::fmt::Debug;

use crate::fileparser::{FileParsingError, ParsingResult};
use scraper::{ElementRef, Html, Selector};
use tracing::info;

#[derive(Debug)]
enum HtmlParsingError {
    ElementAttrNotFound(String),
    ElementNotFound(String),
    Other(String),
}

macro_rules! make_selector {
    ($selector:expr) => {
        match Selector::parse($selector) {
            Ok(val) => val,
            Err(e) => {
                return Err(HtmlParsingError::Other(format!(
                    "Error creating selector: {e:?}"
                )));
            }
        }
    };
}

fn get_title(document: &Html) -> Result<String, HtmlParsingError> {
    info!("== HTML: Parsing title");
    let selector = make_selector!("title");
    document.select(&selector).next().map_or_else(
        || {
            Err(HtmlParsingError::ElementNotFound(
                "Could not find document’s title".into(),
            ))
        },
        |title| {
            let inner = title.inner_html();
            let decorator =
                html2text::render::text_renderer::TrivialDecorator::new();
            Ok(html2text::from_read_with_decorator(
                inner.as_bytes(),
                inner.len(),
                decorator,
            )
            .trim()
            .into())
        },
    )
}

fn get_keywords(document: &Html) -> Result<Vec<String>, HtmlParsingError> {
    info!("== HTML: Parsing keywords");
    let selector = make_selector!(r#"meta[name="keywords"]"#);
    let keywords = document
        .select(&selector)
        .into_iter()
        .filter_map(|e| {
            e.value()
                .attr("content")
                .map(|val| crate::kwparser::split_keywords(&val.to_string()))
        })
        .flatten()
        .collect();
    Ok(keywords)
}

fn get_body(document: &Html) -> Result<String, HtmlParsingError> {
    get_simple_tag(document, "body").map(|body| {
        let decorator =
            html2text::render::text_renderer::TrivialDecorator::new();
        html2text::from_read_with_decorator(
            body.inner_html().as_bytes(),
            body.inner_html().len(),
            decorator,
        )
    })
}

fn get_description(document: &Html) -> Result<String, HtmlParsingError> {
    match get_simple_tag(document, r#"meta[name="description"]"#) {
        Ok(description) => match description.value().attr("content") {
            Some(val) => Ok(val.to_string()),
            None => Err(HtmlParsingError::ElementAttrNotFound(
                r#"Could not find content of description"#.to_string(),
            )),
        },
        Err(e) => Err(e),
    }
}

fn get_simple_tag<'r>(
    document: &'r Html,
    tag: &str,
) -> Result<ElementRef<'r>, HtmlParsingError> {
    info!("== Retrieving HTML tag {tag}");
    let selector = make_selector!(tag);
    document.select(&selector).next().map_or_else(
        || {
            Err(HtmlParsingError::ElementNotFound(format!(
                "Could not find tag {tag}"
            )))
        },
        Ok,
    )
}

/// Parse an HTML file
///
/// Receive an HTML file’s content raw, extract from it its title,
/// keywords, description, and text.
///
/// # Errors
///
/// If any error occurs when parsing the HTML, return it to the caller
/// function. For more information, see [`PdfParsingError`].
///
/// [`HtmlParsingError`]: ./struct.HtmlParsingError.html
pub fn parse(doc: &[u8]) -> ParsingResult {
    let html_string = match std::str::from_utf8(doc) {
        Ok(v) => v,
        Err(e) => {
            return Err(FileParsingError(format!(
                "Could not convert input data to string: {e}"
            )))
        }
    };
    let html = scraper::Html::parse_document(html_string);
    let title = get_title(&html).map_err(FileParsingError::new)?;
    let keywords = get_keywords(&html).map_err(FileParsingError::new)?;
    let body = get_body(&html).map_err(FileParsingError::new)?;
    let subject = get_description(&html).ok();
    info!("====== Subject of file: {subject:?}");
    Ok((title, keywords, body, subject))
}
