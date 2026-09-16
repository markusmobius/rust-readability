use crate::{
    check::is_space,
    dom::{normalize_whitespace, word_count, Tree},
    entities::unescape,
    url::{absolute, Url},
};
use regex::Regex;
use serde_json::Value;
use std::{
    collections::{BTreeMap, HashSet},
    sync::LazyLock,
};

pub(crate) type Metadata = BTreeMap<String, String>;

static TITLE_SEPARATOR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r" [|\-\x{2013}\x{2014}\\/>\x{bb}] ").unwrap());
static TITLE_HIERARCHY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r" [\\/>\x{bb}] ").unwrap());
static TITLE_FINAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(.*)[|\-\x{2013}\x{2014}\\/>\x{bb}] .*").unwrap());
static TITLE_FIRST: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[^|\-\x{2013}\x{2014}\\/>\x{bb}]*[|\-\x{2013}\x{2014}\\/>\x{bb}](.*)").unwrap()
});
static TITLE_ANY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[|\-\x{2013}\x{2014}\\/>\x{bb}]+").unwrap());
static PROPERTY: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)[\t\n\f\r ]*(dc|dcterm|og|article|twitter)[\t\n\f\r ]*:[\t\n\f\r ]*(author|creator|description|title|site_name|published_time|modified_time|image[^\t\n\f\r ]*)[\t\n\f\r ]*").unwrap()
});
static NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[\t\n\f\r ]*(?:(dc|dcterm|article|og|twitter|parsely|weibo:(article|webpage))[\t\n\f\r ]*[-\.:][\t\n\f\r ]*)?(author|creator|pub-date|description|title|site_name|published_time|modified_time|image)[\t\n\f\r ]*$").unwrap()
});
static ARTICLE_TYPES: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^Article|AdvertiserContentArticle|NewsArticle|AnalysisNewsArticle|AskPublicNewsArticle|BackgroundNewsArticle|OpinionNewsArticle|ReportageNewsArticle|ReviewNewsArticle|Report|SatiricalArticle|ScholarlyArticle|MedicalScholarlyArticle|SocialMediaPosting|BlogPosting|LiveBlogPosting|DiscussionForumPosting|TechArticle|APIReference$").unwrap()
});
static CDATA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\t\n\f\r ]*<!\[CDATA\[|\]\]>[\t\n\f\r ]*$").unwrap());
static SCHEMA: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^https?://schema\.org/?$").unwrap());
static FAVICON_SIZE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)([0-9]+)x([0-9]+)").unwrap());

pub(crate) fn lowercase(text: &str) -> String {
    text.chars()
        .map(|character| character.to_lowercase().next().unwrap())
        .collect()
}

pub(crate) fn text_similarity(first: &str, second: &str) -> f64 {
    let delimiter = |character: char| !character.is_ascii_alphanumeric() && character != '_';
    let first = lowercase(first);
    let first: HashSet<_> = first
        .split(delimiter)
        .filter(|token| !token.is_empty())
        .collect();
    let second = lowercase(second);
    let second: Vec<_> = second
        .split(delimiter)
        .filter(|token| !token.is_empty())
        .collect();
    let unique: Vec<_> = second
        .iter()
        .copied()
        .filter(|token| !first.contains(token))
        .collect();
    1.0 - unique.join(" ").chars().count() as f64 / second.join(" ").chars().count() as f64
}

pub(crate) fn article_title(tree: &Tree) -> String {
    let original = tree
        .first(0, "title")
        .map_or_else(String::new, |node| tree.inner_text(node, true));
    let mut title = original.clone();
    let mut hierarchical = false;
    if TITLE_SEPARATOR.is_match(&title) {
        hierarchical = TITLE_HIERARCHY.is_match(&title);
        title = TITLE_FINAL.replace_all(&original, "${1}").into_owned();
        if word_count(&title) < 3 {
            title = TITLE_FIRST.replace_all(&original, "${1}").into_owned();
        }
    } else if title.contains(": ") {
        if !tree
            .all(0, &["h1", "h2"])
            .into_iter()
            .any(|node| tree.text(node).trim_matches(is_space) == title.trim_matches(is_space))
        {
            title = original[original.rfind(':').unwrap() + 1..].into();
            if word_count(&title) < 3 {
                title = original[original.find(':').unwrap() + 1..].into();
            } else if word_count(&original[..original.find(':').unwrap()]) > 5 {
                title = original.clone();
            }
        }
    } else if title.chars().count() > 150 || title.chars().count() < 15 {
        let headings = tree.tagged(0, "h1");
        if headings.len() == 1 {
            title = tree.inner_text(headings[0], true);
        }
    }
    title = normalize_whitespace(&title);
    let count = word_count(&title) as i64;
    if count <= 4
        && (!hierarchical || count != word_count(&TITLE_ANY.replace_all(&original, "")) as i64 - 1)
    {
        title = original;
    }
    title
}

pub(crate) fn json_ld(tree: &Tree) -> Metadata {
    for script in tree.tagged(0, "script") {
        if !tree.nodes[script]
            .attrs
            .iter()
            .any(|attr| attr.key == "type" && attr.value == "application/ld+json")
        {
            continue;
        }
        let content = tree.text(script);
        let content = CDATA.replace_all(&content, "");
        let Some(decoded) = crate::json::parse(&content) else {
            continue;
        };
        let mut parsed = match &decoded.0 {
            Value::Array(items) => {
                let Some(item) = items.iter().find(|item| {
                    item.get("@type")
                        .and_then(Value::as_str)
                        .is_some_and(|kind| ARTICLE_TYPES.is_match(kind))
                }) else {
                    continue;
                };
                item
            }
            Value::Object(_) => &decoded.0,
            _ => continue,
        };
        let context = &parsed["@context"];
        if !context
            .as_str()
            .or_else(|| context.get("@vocab").and_then(Value::as_str))
            .is_some_and(|context| SCHEMA.is_match(context))
        {
            continue;
        }
        if parsed.get("@type").is_none() {
            let Some(graphs) = parsed["@graph"].as_array() else {
                continue;
            };
            if let Some(graph) = graphs.iter().find(|graph| {
                graph
                    .get("@type")
                    .and_then(Value::as_str)
                    .is_some_and(|kind| ARTICLE_TYPES.is_match(kind))
            }) {
                parsed = graph;
            }
        }
        if !parsed["@type"]
            .as_str()
            .is_some_and(|kind| ARTICLE_TYPES.is_match(kind))
        {
            continue;
        }
        let mut result = Metadata::new();
        match (parsed["name"].as_str(), parsed["headline"].as_str()) {
            (Some(name), Some(headline)) if name != headline => {
                let title = article_title(tree);
                let selected = if text_similarity(headline, &title) > 0.75
                    && text_similarity(name, &title).partial_cmp(&0.75)
                        != Some(std::cmp::Ordering::Greater)
                {
                    headline
                } else {
                    name
                };
                result.insert("title".into(), selected.into());
            }
            (Some(name), _) => {
                result.insert("title".into(), name.trim_matches(is_space).into());
            }
            (_, Some(headline)) => {
                result.insert("title".into(), headline.trim_matches(is_space).into());
            }
            _ => {}
        }
        match &parsed["author"] {
            Value::Object(author) => {
                if let Some(name) = author.get("name").and_then(Value::as_str) {
                    result.insert("byline".into(), name.trim_matches(is_space).into());
                }
            }
            Value::Array(authors) => {
                let authors = authors
                    .iter()
                    .filter_map(|author| author.get("name").and_then(Value::as_str))
                    .map(|name| name.trim_matches(is_space))
                    .collect::<Vec<_>>();
                result.insert("byline".into(), authors.join(", "));
            }
            _ => {}
        }
        for (key, value) in [
            ("excerpt", parsed["description"].as_str()),
            (
                "siteName",
                parsed["publisher"].get("name").and_then(Value::as_str),
            ),
        ] {
            if let Some(value) = value {
                result.insert(key.into(), value.trim_matches(is_space).into());
            }
        }
        if let Some(date) = parsed["datePublished"].as_str() {
            result.insert("datePublished".into(), date.into());
        }
        return result;
    }
    Metadata::new()
}

fn favicon(tree: &Tree, base: Option<&Url>) -> String {
    let mut selected = String::new();
    let mut selected_size = -1i64;
    for node in tree.tagged(0, "link") {
        let node = &tree.nodes[node];
        let rel = node.attr("rel").trim_matches(is_space);
        let mime = node.attr("type").trim_matches(is_space);
        let href = node.attr("href").trim_matches(is_space);
        let sizes = node.attr("sizes").trim_matches(is_space);
        if href.is_empty()
            || !rel.contains("icon")
            || (mime != "image/png" && !href.contains(".png"))
        {
            continue;
        }
        let mut size = 0;
        for source in [sizes, href] {
            if let Some(parts) = FAVICON_SIZE.captures(source) {
                if parts[1] == parts[2] {
                    size = parts[1].parse::<i64>().unwrap_or(i64::MAX);
                    break;
                }
            }
        }
        if size > selected_size {
            selected_size = size;
            selected = href.into();
        }
    }
    absolute(&selected, base)
}

fn select(values: &Metadata, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| values.get(*key).filter(|value| !value.is_empty()))
        .cloned()
        .unwrap_or_default()
}

pub(crate) fn article_metadata(tree: &Tree, json_ld: &Metadata, base: Option<&Url>) -> Metadata {
    let mut values = Metadata::new();
    for node in tree.tagged(0, "meta") {
        let node = &tree.nodes[node];
        let content = node.attr("content");
        if content.is_empty() {
            continue;
        }
        let matches: Vec<_> = PROPERTY.find_iter(node.attr("property")).collect();
        for matched in matches.iter().rev() {
            let key: String = lowercase(matched.as_str())
                .chars()
                .filter(|&character| !is_space(character))
                .collect();
            values.insert(key, content.trim_matches(is_space).into());
        }
        let name = node.attr("name");
        if matches.is_empty() && !name.is_empty() && NAME.is_match(name) {
            let key: String = lowercase(name)
                .chars()
                .filter(|&character| !is_space(character))
                .collect();
            values.insert(key.replace('.', ":"), content.trim_matches(is_space).into());
        }
    }
    let mut result = Metadata::new();
    for (key, ld_key, keys) in [
        (
            "title",
            "title",
            &[
                "dc:title",
                "dcterm:title",
                "og:title",
                "weibo:article:title",
                "weibo:webpage:title",
                "title",
                "twitter:title",
                "parsely-title",
            ][..],
        ),
        (
            "byline",
            "byline",
            &["dc:creator", "dcterm:creator", "author", "parsely-author"],
        ),
        (
            "excerpt",
            "excerpt",
            &[
                "dc:description",
                "dcterm:description",
                "og:description",
                "weibo:article:description",
                "weibo:webpage:description",
                "description",
                "twitter:description",
            ],
        ),
        ("siteName", "siteName", &["og:site_name"]),
        (
            "publishedTime",
            "datePublished",
            &[
                "article:published_time",
                "dcterms.available",
                "dcterms.created",
                "dcterms.issued",
                "weibo:article:create_at",
                "parsely-pub-date",
            ],
        ),
        (
            "modifiedTime",
            "dateModified",
            &["article:modified_time", "dcterms.modified"],
        ),
    ] {
        let mut value = select(json_ld, &[ld_key]);
        if value.is_empty() {
            value = select(&values, keys);
        }
        if value.is_empty() {
            if key == "title" {
                value = article_title(tree);
            } else if key == "byline" {
                let author = values
                    .get("article:author")
                    .map(String::as_str)
                    .unwrap_or_default();
                if Url::request(author).is_none() {
                    value = author.into();
                }
            }
        }
        result.insert(key.into(), unescape(&value));
    }
    result.insert(
        "image".into(),
        select(&values, &["og:image", "image", "twitter:image"]),
    );
    result.insert("favicon".into(), favicon(tree, base));
    result
}
