use rust_readability::{parse_dom, Article, Parser, Url};
use serde::{Deserialize, Serialize};
use std::io::{self, BufRead, Write};
use std::time::Instant;

#[derive(Deserialize)]
struct Page {
    file: String,
    url: String,
    html: String,
    with: Vec<String>,
    without: Vec<String>,
}

#[derive(Default, Debug, PartialEq, Serialize)]
struct Counts {
    true_positives: usize,
    false_negatives: usize,
    false_positives: usize,
    true_negatives: usize,
}

impl Counts {
    fn evaluate(&mut self, text: &str, page: &Page) {
        for snippet in &page.with {
            if !text.is_empty() && text.contains(snippet) {
                self.true_positives += 1;
            } else {
                self.false_negatives += 1;
            }
        }
        for snippet in &page.without {
            if !text.is_empty() && text.contains(snippet) {
                self.false_positives += 1;
            } else {
                self.true_negatives += 1;
            }
        }
    }
}

#[derive(Deserialize)]
struct Request {
    #[serde(default)]
    outputs: bool,
}

#[derive(Default, Serialize)]
struct Output {
    file: String,
    text: String,
    html: String,
    title: String,
    byline: String,
    excerpt: String,
    site_name: String,
    image_url: String,
    favicon: String,
    language: String,
    error: String,
}

impl Output {
    fn article(file: &str, text: String, article: &Article, error: String) -> Self {
        let html = if article.node.is_some() {
            article.html().expect("article HTML rendering failed")
        } else {
            String::new()
        };
        Self {
            file: file.to_owned(),
            text,
            html,
            title: article.title().to_owned(),
            byline: article.byline().to_owned(),
            excerpt: article.excerpt(),
            site_name: article.site_name().to_owned(),
            image_url: article.image_url().to_owned(),
            favicon: article.favicon().to_owned(),
            language: article.language().to_owned(),
            error,
        }
    }
}

#[derive(Serialize)]
struct Response {
    elapsed_ns: u128,
    counts: Counts,
    errors: Vec<String>,
    outputs: Vec<Output>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: benchmark CORPUS_JSON")?;
    let pages: Vec<Page> = serde_json::from_reader(io::BufReader::new(std::fs::File::open(path)?))?;
    let documents = pages
        .iter()
        .map(|page| parse_dom(&page.html))
        .collect::<Vec<_>>();
    let urls = pages
        .iter()
        .map(|page| Url::try_request(&page.url))
        .collect::<Result<Vec<_>, _>>()?;
    let stdout = io::stdout();
    let mut writer = io::BufWriter::new(stdout.lock());
    writeln!(writer, "{{\"ready\":{}}}", pages.len())?;
    writer.flush()?;
    for line in io::stdin().lock().lines() {
        let request: Request = serde_json::from_str(&line?)?;
        let mut counts = Counts::default();
        let mut errors = Vec::new();
        let mut outputs = Vec::new();
        let started = Instant::now();
        for ((page, document), url) in pages.iter().zip(&documents).zip(&urls) {
            match Parser::new().parse_dom(document, Some(url)) {
                Ok(article) => {
                    let (text, error) = match article.text() {
                        Ok(text) => (text, String::new()),
                        Err(error) => (String::new(), error.to_string()),
                    };
                    counts.evaluate(&text, page);
                    if !error.is_empty() {
                        errors.push(format!("{}: {error}", page.file));
                    }
                    if request.outputs {
                        outputs.push(Output::article(&page.file, text, &article, error));
                    }
                }
                Err(error) => {
                    counts.evaluate("", page);
                    errors.push(format!("{}: {error}", page.file));
                    if request.outputs {
                        outputs.push(Output {
                            file: page.file.clone(),
                            error: error.to_string(),
                            ..Output::default()
                        });
                    }
                }
            }
        }
        let response = Response {
            elapsed_ns: started.elapsed().as_nanos(),
            counts,
            errors,
            outputs,
        };
        serde_json::to_writer(&mut writer, &response)?;
        writeln!(writer)?;
        writer.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn upstream_snippet_scoring() {
        let page = Page {
            file: String::new(),
            url: String::new(),
            html: String::new(),
            with: vec![
                "article".into(),
                "missing".into(),
                "article".into(),
                "Article".into(),
            ],
            without: vec!["navigation".into(), "footer".into()],
        };
        let mut counts = Counts::default();
        counts.evaluate("article navigation", &page);
        assert_eq!(
            counts,
            Counts {
                true_positives: 2,
                false_negatives: 2,
                false_positives: 1,
                true_negatives: 1,
            }
        );
        counts.evaluate("", &page);
        assert_eq!(counts.false_negatives, 6);
        assert_eq!(counts.true_negatives, 3);
    }
}
