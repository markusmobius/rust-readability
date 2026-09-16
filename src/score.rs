use crate::{check::is_space, dom::Tree, Kind, NodeId};
use aho_corasick::AhoCorasick;
use std::sync::LazyLock;

static POSITIVE: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::new([
        "article",
        "body",
        "content",
        "entry",
        "hentry",
        "h-entry",
        "main",
        "page",
        "pagination",
        "post",
        "text",
        "blog",
        "story",
    ])
    .unwrap()
});
static NEGATIVE: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::new([
        "-ad-",
        "banner",
        "combx",
        "comment",
        "com-",
        "contact",
        "footer",
        "gdpr",
        "masthead",
        "meta",
        "outbrain",
        "promo",
        "related",
        "share",
        "shoutbox",
        "sidebar",
        "skyscraper",
        "sponsor",
        "shopping",
        "tags",
        "widget",
    ])
    .unwrap()
});
static BYLINE: LazyLock<AhoCorasick> = LazyLock::new(|| {
    AhoCorasick::builder()
        .ascii_case_insensitive(true)
        .build(["byline", "author", "dateline", "writtenby", "p-author"])
        .unwrap()
});

static NEGATIVE_BOUNDARY: LazyLock<regex::bytes::Regex> =
    LazyLock::new(|| regex::bytes::Regex::new(r"(?-u:(.)?(hidden|hid|d-none)(.)?)").unwrap());

#[derive(Default)]
pub(crate) struct CharCounter {
    pub total: i64,
    space: bool,
    seen: bool,
}

impl CharCounter {
    pub fn count(&mut self, character: char) {
        if is_space(character) {
            self.space = true;
            return;
        }
        self.total += if self.space && self.seen { 2 } else { 1 };
        self.space = false;
        self.seen = true;
    }

    pub fn reset_context(&mut self) {
        self.space = false;
        self.seen = false;
    }

    pub fn append(&mut self, summary: &TextSummary) {
        if summary.characters > 0 {
            self.total +=
                summary.characters + i64::from(self.seen && (self.space || summary.leading_space));
            self.space = summary.trailing_space;
            self.seen = true;
        } else if summary.has_input {
            self.space = true;
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TextSummary {
    characters: i64,
    pub commas: i64,
    leading_space: bool,
    trailing_space: bool,
    has_input: bool,
}

impl TextSummary {
    pub(crate) fn cache_bits(&self) -> Option<u64> {
        let characters = u64::try_from(self.characters).ok()?;
        let commas = u64::try_from(self.commas).ok()?;
        if characters > u64::from(u32::MAX) || commas >= 1 << 29 {
            return None;
        }
        Some(
            characters
                | (commas << 32)
                | (u64::from(self.leading_space) << 61)
                | (u64::from(self.trailing_space) << 62)
                | (u64::from(self.has_input) << 63),
        )
    }

    pub(crate) fn from_cache_bits(bits: u64) -> Self {
        Self {
            characters: (bits & u64::from(u32::MAX)) as i64,
            commas: ((bits >> 32) & ((1 << 29) - 1)) as i64,
            leading_space: bits & (1 << 61) != 0,
            trailing_space: bits & (1 << 62) != 0,
            has_input: bits & (1 << 63) != 0,
        }
    }

    pub fn new(text: &str) -> Self {
        let mut counter = CharCounter::default();
        let mut commas = 0;
        for character in text.chars() {
            counter.count(character);
            commas += i64::from(comma(character));
        }
        Self {
            characters: counter.total,
            commas,
            leading_space: text.chars().next().is_some_and(is_space),
            trailing_space: counter.space,
            has_input: !text.is_empty(),
        }
    }
}

pub(crate) fn comma(character: char) -> bool {
    matches!(
        character,
        ',' | '\u{60c}'
            | '\u{fe50}'
            | '\u{fe10}'
            | '\u{fe11}'
            | '\u{2e41}'
            | '\u{2e34}'
            | '\u{2e32}'
            | '\u{ff0c}'
    )
}

pub(crate) fn counts(tree: &Tree, root: NodeId) -> (i64, i64) {
    let mut chars = CharCounter::default();
    let mut commas = 0;
    let mut pending = vec![root];
    while let Some(node) = pending.pop() {
        if tree.nodes[node].kind == Kind::Text {
            let summary = tree.nodes[node].text_summary();
            chars.append(&summary);
            commas += summary.commas;
        } else {
            pending.extend(tree.nodes[node].children.iter().rev());
        }
    }
    (chars.total, commas)
}

pub(crate) fn link_coefficient(tree: &Tree, node: NodeId) -> f64 {
    let href = tree.nodes[node].attr("href").trim_matches(is_space);
    if href.len() > 1 && href.starts_with('#') {
        0.3
    } else {
        1.0
    }
}

pub(crate) fn link_density(tree: &Tree, root: NodeId) -> f64 {
    let mut chars = CharCounter::default();
    let mut links = Vec::<CharCounter>::new();
    let mut weighted = 0.0;
    let mut pending: Vec<(NodeId, Option<usize>, bool)> = vec![(root, None, false)];
    while let Some((node, mut link, closing)) = pending.pop() {
        if closing {
            weighted += links[link.unwrap()].total as f64 * link_coefficient(tree, node);
            continue;
        }
        if tree.nodes[node].kind == Kind::Text {
            let summary = tree.nodes[node].text_summary();
            chars.append(&summary);
            if let Some(link) = link {
                links[link].append(&summary);
            }
            continue;
        }
        if tree.nodes[node].kind == Kind::Element && tree.nodes[node].tag == "a" {
            link = Some(links.len());
            links.push(CharCounter::default());
            pending.push((node, link, true));
        }
        pending.extend(
            tree.nodes[node]
                .children
                .iter()
                .rev()
                .map(|&child| (child, link, false)),
        );
    }
    if chars.total == 0 {
        0.0
    } else {
        weighted / chars.total as f64
    }
}

pub(crate) fn positive(text: &str) -> bool {
    POSITIVE.is_match(text)
}

pub(crate) fn negative(text: &str) -> bool {
    if NEGATIVE.is_match(text) {
        return true;
    }
    NEGATIVE_BOUNDARY
        .captures_iter(text.as_bytes())
        .any(|captures| {
            let whole = captures.get(0).unwrap();
            let before = captures
                .get(1)
                .map(|capture| capture.as_bytes())
                .unwrap_or_default();
            let after = captures
                .get(3)
                .map(|capture| capture.as_bytes())
                .unwrap_or_default();
            (whole.start() == 0
                && whole.end() == text.len()
                && before.is_empty()
                && after.is_empty())
                || (whole.end() == text.len() && before == b" " && after.is_empty())
                || (whole.start() == 0 && before.is_empty() && after == b" ")
                || (before == b" " && after == b" ")
        })
}

pub(crate) fn class_weight(tree: &Tree, node: NodeId, use_classes: bool) -> i64 {
    if !use_classes {
        return 0;
    }
    let mut weight = 0;
    for value in [tree.nodes[node].attr("class"), tree.nodes[node].attr("id")] {
        let value = value.to_ascii_lowercase();
        if negative(&value) {
            weight -= 25;
        }
        if positive(&value) {
            weight += 25;
        }
    }
    weight
}

pub(crate) fn byline(tree: &Tree, node: NodeId, matched: &str) -> bool {
    tree.nodes[node].attr("rel") == "author"
        || tree.nodes[node].attr("itemprop").contains("author")
        || BYLINE.is_match(matched)
}

pub(crate) fn has_score(tree: &Tree, node: NodeId) -> bool {
    tree.nodes[node].has_attr("data-readability-score")
}

pub(crate) fn get_score(tree: &Tree, node: NodeId) -> f64 {
    tree.nodes[node].score()
}

pub(crate) fn set_score(tree: &mut Tree, node: NodeId, score: f64) {
    let value = if score == f64::INFINITY {
        "+Inf".into()
    } else if score == f64::NEG_INFINITY {
        "-Inf".into()
    } else {
        smol_str::format_smolstr!("{score:.4}")
    };
    tree.nodes[node].set_attr("data-readability-score", &value);
}

pub(crate) fn initialize(tree: &mut Tree, node: NodeId, use_classes: bool) {
    let bonus = match tree.nodes[node].tag.as_str() {
        "div" => 5,
        "pre" | "td" | "blockquote" => 3,
        "address" | "ol" | "ul" | "dl" | "dd" | "dt" | "li" | "form" => -3,
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "th" => -5,
        _ => 0,
    };
    let value = class_weight(tree, node, use_classes) + bonus;
    set_score(tree, node, value as f64);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Document;

    #[test]
    fn summary_cache_encoding_preserves_values_or_falls_back() {
        for characters in [0, 1, i64::from(u32::MAX), i64::from(u32::MAX) + 1, i64::MAX] {
            for commas in [0, 1, (1 << 29) - 1, 1 << 29, i64::MAX] {
                for flags in 0..8 {
                    let summary = TextSummary {
                        characters,
                        commas,
                        leading_space: flags & 1 != 0,
                        trailing_space: flags & 2 != 0,
                        has_input: flags & 4 != 0,
                    };
                    if let Some(bits) = summary.cache_bits() {
                        let restored = TextSummary::from_cache_bits(bits);
                        assert_eq!(restored.characters, characters);
                        assert_eq!(restored.commas, commas);
                        assert_eq!(restored.leading_space, summary.leading_space);
                        assert_eq!(restored.trailing_space, summary.trailing_space);
                        assert_eq!(restored.has_input, summary.has_input);
                    } else {
                        assert!(characters > i64::from(u32::MAX) || commas >= 1 << 29);
                    }
                }
            }
        }
    }

    #[test]
    fn inline_score_formatting_preserves_canonical_values() {
        let mut tree = Tree::new(Document::parse("<p></p>"));
        let node = tree.first(0, "p").unwrap();
        for value in [
            0.0,
            -0.0,
            1.0 / 3.0,
            1.23445,
            1.23455,
            -1.23445,
            f64::MIN_POSITIVE,
            f64::MAX,
            -f64::MAX,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NAN,
        ] {
            let expected = if value == f64::INFINITY {
                "+Inf".to_owned()
            } else if value == f64::NEG_INFINITY {
                "-Inf".to_owned()
            } else {
                format!("{value:.4}")
            };
            set_score(&mut tree, node, value);
            assert_eq!(tree.nodes[node].attr("data-readability-score"), expected);
            let expected = expected.parse::<f64>().unwrap();
            let actual = get_score(&tree, node);
            assert!(actual.to_bits() == expected.to_bits() || actual.is_nan() && expected.is_nan());
        }
    }

    #[test]
    fn text_summaries_preserve_scalar_counter_context() {
        let inputs = [
            "", "a", "ab", " ", "\t\r\n", "a  b", ",", "\u{60c}", "\u{85}", "\u{a0}", "\u{200a}",
            "\u{3000}", "e\u{301}", "\u{5b57}", "\u{200b}",
        ];
        for first in inputs {
            for second in inputs {
                for third in inputs {
                    for reset in [false, true] {
                        let mut scalar = CharCounter::default();
                        let mut summarized = CharCounter::default();
                        let mut commas = 0;
                        for (index, value) in [first, second, third].into_iter().enumerate() {
                            if reset && index == 1 {
                                scalar.reset_context();
                                summarized.reset_context();
                            }
                            for character in value.chars() {
                                scalar.count(character);
                            }
                            let summary = TextSummary::new(value);
                            summarized.append(&summary);
                            commas += summary.commas;
                            assert_eq!(
                                (scalar.total, scalar.space, scalar.seen),
                                (summarized.total, summarized.space, summarized.seen)
                            );
                        }
                        assert_eq!(
                            commas,
                            [first, second, third]
                                .concat()
                                .chars()
                                .filter(|&character| comma(character))
                                .count() as i64
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn scoring_primitives() {
        let mut tree = Tree::new(Document::parse("<div class='ARTICLE hidden' id='main'><p>  one\u{a0}<b>two,</b> three <a href='#section'>more</a> </p></div>"));
        let node = tree.first(0, "div").unwrap();
        assert_eq!(counts(&tree, node), (19, 1));
        assert_eq!(class_weight(&tree, node, true), 25);
        assert_eq!(class_weight(&tree, node, false), 0);
        assert_eq!(link_density(&tree, node), 1.2 / 19.0);
        initialize(&mut tree, node, true);
        assert_eq!(get_score(&tree, node), 30.0);
        set_score(&mut tree, node, 1.0 / 3.0);
        assert_eq!(get_score(&tree, node), 0.3333);
        assert!(negative("hidden"));
        assert!(negative("x hid y"));
        assert!(!negative("x-hidden-y"));
        assert!(!negative("hiddenly"));
    }
}
