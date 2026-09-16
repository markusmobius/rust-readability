use crate::{Document, Node};
use aho_corasick::AhoCorasick;
use regex::Regex;
use std::{collections::HashSet, sync::LazyLock};

static DISPLAY_NONE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)display[\t\n\f\r ]*:[\t\n\f\r ]*none").unwrap());
static VISIBILITY_HIDDEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)visibility[\t\n\f\r ]*:[\t\n\f\r ]*hidden").unwrap());

static UNLIKELY: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .build([
            "-ad-",
            "ai2html",
            "banner",
            "breadcrumbs",
            "combx",
            "comment",
            "community",
            "cover-wrap",
            "disqus",
            "extra",
            "footer",
            "gdpr",
            "header",
            "legends",
            "menu",
            "related",
            "remark",
            "replies",
            "rss",
            "shoutbox",
            "sidebar",
            "skyscraper",
            "social",
            "sponsor",
            "supplemental",
            "ad-break",
            "agegate",
            "pagination",
            "pager",
            "popup",
            "yom-remote",
        ])
        .unwrap()
});
static CANDIDATE: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .build([
            "and", "article", "body", "column", "content", "main", "mathjax", "shadow",
        ])
        .unwrap()
});

pub(crate) fn is_space(character: char) -> bool {
    matches!(character, '\u{9}'..='\u{d}' | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

pub(crate) fn probably_visible(node: &Node) -> bool {
    !DISPLAY_NONE.is_match(node.attr("style"))
        && !VISIBILITY_HIDDEN.is_match(node.attr("style"))
        && !node.has_attr("hidden")
        && (node.attr("aria-hidden") != "true" || node.attr("class").contains("fallback-image"))
}

pub(crate) fn unlikely(input: &str) -> bool {
    UNLIKELY.is_match(input)
}

pub(crate) fn maybe_candidate(input: &str) -> bool {
    CANDIDATE.is_match(input)
}

pub fn check_document(document: &Document) -> bool {
    let mut nodes = document
        .elements(0)
        .into_iter()
        .filter(|&node| matches!(document.nodes[node].tag.as_str(), "p" | "pre" | "article"))
        .collect::<Vec<_>>();
    let mut seen = HashSet::new();
    for line_break in document.tagged(0, "br") {
        if let Some(parent) = document.nodes[line_break].parent {
            if document.nodes[parent].tag == "div" && seen.insert(parent) {
                nodes.push(parent);
            }
        }
    }
    let mut score = 0.0;
    for index in nodes {
        let node = &document.nodes[index];
        if !probably_visible(node) {
            continue;
        }
        let marker = format!("{} {}", node.attr("class"), node.attr("id"));
        if unlikely(&marker) && !maybe_candidate(&marker) {
            continue;
        }
        if node.tag == "p" {
            let mut parent = node.parent;
            let mut in_list = false;
            while let Some(ancestor) = parent {
                if document.nodes[ancestor].tag == "li" {
                    in_list = true;
                    break;
                }
                parent = document.nodes[ancestor].parent;
            }
            if in_list {
                continue;
            }
        }
        let length = document.text(index).trim_matches(is_space).len();
        if length >= 140 {
            score += ((length - 140) as f64).sqrt();
            if score > 20.0 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readability_boundaries() {
        for (source, expected) in [
            (format!("<p>{}</p>", "x".repeat(540)), false),
            (format!("<p>{}</p>", "x".repeat(541)), true),
            (format!("<p>{}</p>", "\u{5b57}".repeat(181)), true),
            (format!("<p hidden>{}</p>", "x".repeat(600)), false),
            (
                format!("<p style='display: none'>{}</p>", "x".repeat(600)),
                false,
            ),
            (format!("<li><p>{}</p></li>", "x".repeat(600)), false),
            (format!("<p class='sidebar'>{}</p>", "x".repeat(600)), false),
            (
                format!("<p class='sidebar article'>{}</p>", "x".repeat(600)),
                true,
            ),
            (format!("<div>{}<br><br></div>", "x".repeat(300)), false),
            (format!("<div>{}<br><br></div>", "x".repeat(541)), true),
            (
                format!(
                    "<p aria-hidden='true' class='fallback-image'>{}</p>",
                    "x".repeat(600)
                ),
                true,
            ),
        ] {
            let document = Document::parse(&source);
            let original = document.clone();
            assert_eq!(check_document(&document), expected, "{source}");
            assert_eq!(document, original);
        }
    }
}
